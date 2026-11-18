#!/bin/sh
# pkg's installer for macOS and Linux:
#
#     curl -fsSL https://gotpkg.dev | sh
#     curl -fsSL https://gotpkg.dev | sh -s -- --version 0.3.0 --no-modify-path --dry-run
#
# It downloads the pkg archive for this machine and the SHA256SUMS manifest from one GitHub
# release, checks the archive against the manifest, installs pkg at ~/.local/bin/pkg, and puts
# ~/.local/bin on PATH in the startup file of the shell $SHELL names. It never runs as root, uses
# sudo, or installs mise.
#
# Trust: SHA256SUMS comes from the same release as the archive, so the check detects a corrupted
# download, not a compromised release. Verify an archive independently with
# `gh attestation verify <archive> --repo gotpkg/pkg`.
#
# PKG_RELEASES_URL replaces https://github.com/gotpkg/pkg/releases, for a mirror.
#
# Nothing runs until `main` on the last line, so a download cut short does nothing.

# The release this script belongs to: `cargo xtask installer` writes it in when it builds the
# release's install.sh. A copy from a checkout names no release and needs --version.
release_version="@PKG_RELEASE_VERSION@"

set -u

releases_url=${PKG_RELEASES_URL:-https://github.com/gotpkg/pkg/releases}
block_start='# >>> pkg >>>'
block_end='# <<< pkg <<<'

usage() {
    cat <<'EOF'
usage: curl -fsSL https://gotpkg.dev | sh -s -- [options]

Installs pkg at ~/.local/bin/pkg and puts ~/.local/bin on PATH.

options:
  --version <version>  install that released version, such as 0.3.0
  --no-modify-path     leave startup files alone and print the line to add
  --dry-run            report what would change; download and change nothing
  --help               show this help
EOF
}

# fail <message> [<hint>...]
fail() {
    printf 'error: %s\n' "$1" >&2
    shift
    for hint in "$@"; do
        printf 'hint: %s\n' "$hint" >&2
    done
    exit 1
}

usage_error() {
    printf 'error: %s\n\n' "$1" >&2
    usage >&2
    exit 2
}

# The result goes to stdout; progress goes to stderr.
say() {
    printf '%s\n' "$@"
}

progress() {
    printf '%s\n' "$1" >&2
}

# The path with the home directory written as ~.
tilde() {
    case $1 in
        "$HOME"/*) printf '%s/%s\n' '~' "${1#"$HOME"/}" ;;
        *) printf '%s\n' "$1" ;;
    esac
}

parse_arguments() {
    requested_version=
    modify_path=true
    dry_run=false
    while [ $# -gt 0 ]; do
        case $1 in
            --version)
                if [ $# -lt 2 ] || [ -z "$2" ]; then
                    usage_error "--version needs a version, such as 0.3.0"
                fi
                requested_version=$2
                shift
                ;;
            --no-modify-path) modify_path=false ;;
            --dry-run) dry_run=true ;;
            -h | --help)
                usage
                exit 0
                ;;
            *) usage_error "unknown option: $1" ;;
        esac
        shift
    done
}

refuse_root() {
    if [ "$(id -u)" = 0 ]; then
        fail "the installer does not run as root" \
            "run it as the user who will use pkg; it installs to that user's ~/.local/bin"
    fi
}

choose_version() {
    if [ -n "$requested_version" ]; then
        version=${requested_version#v}
    else
        case $release_version in
            [0-9]*) version=$release_version ;;
            *) fail "this copy of the installer belongs to no release" \
                "name the version to install: sh install.sh --version <version>" ;;
        esac
    fi
    case $version in
        '' | [!0-9]* | *[!0-9A-Za-z.+-]*) usage_error "not a pkg version: $requested_version" ;;
    esac
}

# Releases ship one archive per Rust target triple; Linux builds are static musl executables.
detect_target() {
    system=$(uname -s)
    machine=$(uname -m)
    case $system in
        Darwin) platform=apple-darwin ;;
        Linux) platform=unknown-linux-musl ;;
        *) fail "the installer supports macOS and Linux, not $system" \
            "on Windows, download pkg-x86_64-pc-windows-msvc.zip from https://github.com/gotpkg/pkg/releases" ;;
    esac
    case $machine in
        arm64 | aarch64) cpu=aarch64 ;;
        x86_64 | amd64) cpu=x86_64 ;;
        *) fail "pkg has no release for $system on $machine" \
            "releases cover arm64 and x86_64 machines" ;;
    esac
    target=$cpu-$platform
}

locate_destination() {
    case ${HOME:-} in
        /*) ;;
        *) fail "HOME is not set to an absolute path, so there is no ~/.local/bin to install to" ;;
    esac
    bin_dir=$HOME/.local/bin
    destination=$bin_dir/pkg
}

# Sets installed_version when the destination holds pkg; refuses anything else there except a
# broken link.
inspect_destination() {
    installed_version=
    broken_link=false
    if [ -L "$destination" ]; then
        if [ -e "$destination" ]; then
            fail "$destination is a link, so mise manages pkg on this machine" \
                "update pkg with: pkg update github:gotpkg/pkg"
        fi
        broken_link=true
        return 0
    fi
    if [ ! -e "$destination" ]; then
        return 0
    fi
    if [ -f "$destination" ] && [ -x "$destination" ]; then
        reported=$("$destination" --version </dev/null 2>/dev/null | head -n 1)
        case $reported in
            "pkg "[0-9]*)
                installed_version=${reported#pkg }
                return 0
                ;;
        esac
    fi
    fail "$destination is not pkg, so the installer leaves it alone" \
        "move it aside, then run the installer again"
}

# path_setup is `ready` when ~/.local/bin is on PATH already, `files` when the installer adds it
# to the login shell's startup files, and `manual` when the person adds path_line themselves.
choose_path_setup() {
    shell_name=${SHELL:-}
    shell_name=${shell_name##*/}
    # shellcheck disable=SC2016 # $HOME and $PATH expand when the startup file runs.
    path_line='export PATH="$HOME/.local/bin:$PATH"'
    case $shell_name in
        zsh) load_file=${ZDOTDIR:-$HOME}/.zshenv ;;
        bash) load_file=$HOME/.bashrc ;;
        fish)
            load_file=${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/pkg.fish
            # shellcheck disable=SC2016 # $HOME and $PATH expand when fish reads the file.
            path_line='set -gx PATH "$HOME/.local/bin" $PATH'
            ;;
        *) load_file= ;;
    esac
    case :${PATH:-}: in
        *:"$bin_dir":* | *:"$bin_dir"/:*) path_setup=ready ;;
        *)
            if [ "$modify_path" = true ] && [ -n "$load_file" ]; then
                path_setup=files
            else
                path_setup=manual
            fi
            ;;
    esac
}

# Runs "$@" <file> for each startup file that every new shell of the login shell reads.
each_startup_file() {
    "$@" "$load_file"
    if [ "$shell_name" = bash ]; then
        if [ -e "$HOME/.bash_profile" ]; then
            "$@" "$HOME/.bash_profile"
        else
            "$@" "$HOME/.profile"
        fi
    fi
}

add_path_block() {
    if [ -f "$1" ] && grep -qxF "$block_start" "$1"; then
        say "$1 already puts ~/.local/bin on PATH"
        return 0
    fi
    if [ "$dry_run" = true ]; then
        say "Would add ~/.local/bin to PATH in $1"
        return 0
    fi
    mkdir -p "${1%/*}" || fail "cannot create the directory for $1"
    if [ -s "$1" ] && [ -n "$(tail -c 1 "$1")" ]; then
        printf '\n' >>"$1" || fail "cannot add ~/.local/bin to PATH in $1"
    fi
    printf '%s\n' "$block_start" "$path_line" "$block_end" >>"$1" ||
        fail "cannot add ~/.local/bin to PATH in $1"
    say "Added ~/.local/bin to PATH in $1"
}

require_tools() {
    if command -v curl >/dev/null 2>&1; then
        downloader=curl
    elif command -v wget >/dev/null 2>&1; then
        downloader=wget
    else
        fail "the installer needs curl or wget to download pkg"
    fi
    if ! command -v tar >/dev/null 2>&1; then
        fail "the installer needs tar to unpack pkg's archive"
    fi
    if command -v sha256sum >/dev/null 2>&1; then
        hasher='sha256sum'
    elif command -v shasum >/dev/null 2>&1; then
        hasher='shasum -a 256'
    else
        fail "the installer needs sha256sum or shasum to check pkg's archive"
    fi
}

# download <url> <file>
download() {
    case $downloader in
        curl) curl --fail --silent --show-error --location --proto-redir =https --tlsv1.2 \
            --retry 3 --output "$2" "$1" ;;
        wget) wget -q -O "$2" "$1" ;;
    esac
}

install_pkg() {
    archive=pkg-$target.tar.gz
    release_url=$releases_url/download/v$version
    work=$(mktemp -d "${TMPDIR:-/tmp}/pkg-install.XXXXXX") ||
        fail "cannot create a temporary directory"
    progress "Downloading pkg $version for $target from $release_url"
    download "$release_url/$archive" "$work/$archive" ||
        fail "cannot download $release_url/$archive" \
            "check that pkg $version is released: https://github.com/gotpkg/pkg/releases"
    download "$release_url/SHA256SUMS" "$work/SHA256SUMS" ||
        fail "cannot download $release_url/SHA256SUMS"
    verify_archive
    unpack_archive
    replace_destination
}

verify_archive() {
    expected=$(awk -v name="$archive" '$2 == name { print $1 }' "$work/SHA256SUMS")
    if [ -z "$expected" ]; then
        fail "the SHA256SUMS of pkg $version lists no $archive"
    fi
    # shellcheck disable=SC2086 # $hasher is a command and its options.
    actual=$($hasher "$work/$archive" | awk '{ print $1 }')
    if [ "$actual" != "$expected" ]; then
        fail "$archive does not match its checksum in SHA256SUMS, so the download is corrupted" \
            "nothing was installed; run the installer again"
    fi
}

# The archive must hold exactly pkg-<target>/pkg, a regular file.
unpack_archive() {
    member=pkg-$target/pkg
    members=$(tar -tzf "$work/$archive") || fail "cannot list the files in $archive"
    if [ "$members" != "$member" ]; then
        fail "$archive holds files other than $member, so the installer will not unpack it"
    fi
    if ! mkdir "$work/unpacked" || ! tar -xzf "$work/$archive" -C "$work/unpacked" "$member"; then
        fail "cannot unpack $archive"
    fi
    unpacked=$work/unpacked/$member
    if [ -L "$unpacked" ] || [ ! -f "$unpacked" ]; then
        fail "$member in $archive is not a regular file"
    fi
}

# Copies the new pkg next to the destination and renames it over the old one, so the old pkg
# keeps working until the new one is complete and has reported its version.
replace_destination() {
    mkdir -p "$bin_dir" || fail "cannot create $bin_dir"
    staged=$(mktemp "$bin_dir/.pkg.XXXXXX") || fail "cannot create a file in $bin_dir"
    { cp "$unpacked" "$staged" && chmod 755 "$staged"; } || fail "cannot copy pkg into $bin_dir"
    reported=$("$staged" --version </dev/null 2>/dev/null | head -n 1)
    if [ "$reported" != "pkg $version" ]; then
        fail "the downloaded pkg reports \"$reported\" instead of \"pkg $version\"" \
            "nothing was installed"
    fi
    mv -f "$staged" "$destination" || fail "cannot move pkg into place at $destination"
    staged=
}

cleanup() {
    if [ -n "${staged:-}" ]; then
        rm -f "$staged"
    fi
    if [ -n "${work:-}" ]; then
        rm -rf "$work"
    fi
}

replacing() {
    if [ -n "$installed_version" ]; then
        printf ', replacing pkg %s' "$installed_version"
    elif [ "$broken_link" = true ]; then
        printf ', replacing a broken link'
    fi
}

print_next_steps() {
    say '' 'Next steps:'
    case $path_setup in
        files)
            say "- Load the new PATH in this terminal (new terminals already have it):" \
                "    source $(tilde "$load_file")"
            ;;
        manual)
            say "- Put ~/.local/bin on PATH: add this line to your shell's startup file, and run it in this terminal:" \
                "    $path_line"
            ;;
    esac
    say "- Restore a configuration from your dotfiles repository:" \
        "    pkg bootstrap --from <your dotfiles repository> --directory ~/.dotfiles --config-dir <its mise directory>" \
        "  or start fresh:" \
        "    pkg setup"
}

main() {
    parse_arguments "$@"
    refuse_root
    choose_version
    detect_target
    locate_destination
    inspect_destination
    choose_path_setup
    staged=
    work=
    trap cleanup EXIT
    trap 'exit 1' HUP INT TERM

    if [ "$installed_version" = "$version" ]; then
        say "pkg $version is already installed at $destination"
    elif [ "$dry_run" = true ]; then
        require_tools
        say "Would install pkg $version ($target) at $destination$(replacing)" \
            "  from $releases_url/download/v$version/pkg-$target.tar.gz"
    else
        require_tools
        install_pkg
        say "Installed pkg $version ($target) at $destination$(replacing)"
    fi
    if [ "$path_setup" = files ]; then
        each_startup_file add_path_block
    fi
    if [ "$dry_run" = true ]; then
        say "Dry run: nothing was downloaded or changed."
    else
        print_next_steps
    fi
}

main "$@"
