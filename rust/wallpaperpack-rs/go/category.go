package wallpaperpack

// Category is the element type of Pack.Categories. rust-anything generates the
// reference to it but not its declaration, so it lives here: a separate file
// survives `rust-anything refresh`, which rewrites wallpaperpack.go wholesale.

type Category struct {
	ID    string  `json:"id"`
	Name  string  `json:"name"`
	Items []Entry `json:"items"`
}
