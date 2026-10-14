package system

import (
	"os"
	"sync"

	"github.com/yinorg/Yin-Panel/backend/internal/biz/theme"
)

// multipartMemoryBudget is how much of a theme upload net/http keeps in memory
// before it spools the rest to a temporary file. A maximum-size package is read
// twice — once by the multipart reader and once by the read that follows — so a
// large budget would hold the archive in memory twice over.
const multipartMemoryBudget = 1 << 20

// multipartSpoolAvailable reports whether the multipart reader can fall back to
// a temporary file. Without a writable temp directory a small budget would make
// every large upload fail, so the budget widens back to the in-memory maximum.
var multipartSpoolAvailable = sync.OnceValue(func() bool {
	file, err := os.CreateTemp("", "yin-panel-upload-probe-*")
	if err != nil {
		return false
	}
	name := file.Name()
	_ = file.Close()
	_ = os.Remove(name)
	return true
})

// uploadMemoryBudget returns the in-memory limit for a theme package upload.
func uploadMemoryBudget() int64 {
	if multipartSpoolAvailable() {
		return multipartMemoryBudget
	}
	return theme.MaxArchive + (1 << 20)
}
