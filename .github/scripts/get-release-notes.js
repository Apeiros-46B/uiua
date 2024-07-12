const fs = require("fs")
let md = fs.readFileSync("changelog.md", "utf-8")
if (md.includes(`## ${process.argv[2]}`)) {
    let notes = md
        .replaceAll(process.argv[2], "\0VER\0")
        .match(/(?<=^|\n)## \0VER\0 .*?(?=\n## |$)/s)[0]
        .replaceAll("\0VER\0", process.argv[2])
        .trim()
    console.log(notes)
} else {
    console.log("You can find the working changelog [here](https://www.uiua.org/docs/changelog).")
}
