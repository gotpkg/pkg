{ sub(/\r$/, "") }
/^HTTP\// { status = $2; build = "" }
tolower($1) == "x-build:" { build = $2 }
END { print (status ? status : "000"), build }
