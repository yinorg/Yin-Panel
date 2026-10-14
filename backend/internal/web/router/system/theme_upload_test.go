package system

import (
	"bytes"
	"io"
	"mime/multipart"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/yinorg/Yin-Panel/backend/internal/biz/theme"
)

func TestUploadMemoryBudgetKeepsLargePackagesOutOfMemory(t *testing.T) {
	if !multipartSpoolAvailable() {
		t.Skip("no writable temp directory: the in-memory fallback is the expected behaviour")
	}
	budget := uploadMemoryBudget()
	if budget != multipartMemoryBudget {
		t.Fatalf("uploadMemoryBudget() = %d, want %d", budget, multipartMemoryBudget)
	}
	if budget >= theme.MaxArchive {
		t.Fatalf("uploadMemoryBudget() = %d must stay well below theme.MaxArchive = %d", budget, theme.MaxArchive)
	}
}

// A budget far below the package size is only safe if net/http really spools the
// surplus to a temporary file and the part still reads back intact.
func TestUploadMemoryBudgetStillParsesAPackageLargerThanTheBudget(t *testing.T) {
	payload := bytes.Repeat([]byte("yin-panel"), int(multipartMemoryBudget))

	var body bytes.Buffer
	writer := multipart.NewWriter(&body)
	part, err := writer.CreateFormFile("package", "package.zip")
	if err != nil {
		t.Fatalf("CreateFormFile: %v", err)
	}
	if _, err := part.Write(payload); err != nil {
		t.Fatalf("write part: %v", err)
	}
	if err := writer.Close(); err != nil {
		t.Fatalf("close writer: %v", err)
	}

	request := httptest.NewRequest(http.MethodPost, "/api/theme/v2/admin/install", &body)
	request.Header.Set("Content-Type", writer.FormDataContentType())
	if err := request.ParseMultipartForm(uploadMemoryBudget()); err != nil {
		t.Fatalf("ParseMultipartForm with a %d byte budget: %v", multipartMemoryBudget, err)
	}
	file, _, err := request.FormFile("package")
	if err != nil {
		t.Fatalf("FormFile: %v", err)
	}
	defer file.Close()
	readBack, err := io.ReadAll(file)
	if err != nil {
		t.Fatalf("read part: %v", err)
	}
	if !bytes.Equal(readBack, payload) {
		t.Fatalf("read back %d bytes, want %d", len(readBack), len(payload))
	}
}
