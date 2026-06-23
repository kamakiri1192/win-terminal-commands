fn main() {
    // `gunzip` is `gzip` in decompress mode.
    std::process::exit(win_terminal_commands::gzip::run("gunzip", true));
}
