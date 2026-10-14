package theme

import (
	"fmt"
	"testing"
)

func resetPackagePreviewsV2(t *testing.T) {
	t.Helper()
	packagePreviewsV2.Lock()
	packagePreviewsV2.items = map[string]previewPackageV2{}
	packagePreviewsV2.Unlock()
	t.Cleanup(func() {
		packagePreviewsV2.Lock()
		packagePreviewsV2.items = map[string]previewPackageV2{}
		packagePreviewsV2.Unlock()
	})
}

func TestSavePackagePreviewV2CapsRetainedPreviews(t *testing.T) {
	resetPackagePreviewsV2(t)

	tokens := make([]string, 0, maxPackagePreviewsV2+2)
	for index := 0; index < maxPackagePreviewsV2+2; index++ {
		token, err := SavePackagePreviewV2(&PackageV2{Revision: fmt.Sprintf("rev-%d", index)})
		if err != nil {
			t.Fatalf("SavePackagePreviewV2: %v", err)
		}
		tokens = append(tokens, token)
	}

	packagePreviewsV2.Lock()
	retained := len(packagePreviewsV2.items)
	packagePreviewsV2.Unlock()
	if retained != maxPackagePreviewsV2 {
		t.Fatalf("retained previews = %d, want %d", retained, maxPackagePreviewsV2)
	}

	if _, err := GetPackagePreviewV2(tokens[len(tokens)-1]); err != nil {
		t.Fatalf("the newest preview was evicted: %v", err)
	}
	if _, err := GetPackagePreviewV2(tokens[0]); err == nil {
		t.Fatal("the oldest preview should have been evicted")
	}
}
