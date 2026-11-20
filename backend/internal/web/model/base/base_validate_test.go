package base

import (
	"strings"
	"testing"
)

type orderedParams struct {
	Mail     string `validate:"required,email"`
	Password string `validate:"required,max=50"`
}

// The API reports validation errors in declaration order. `Translate` hands
// back a map, so walking it directly produced a different message for the same
// request on different runs.
func TestValidateInputStructKeepsDeclarationOrder(t *testing.T) {
	want := " Mail为必填字段 Password为必填字段"
	for i := 0; i < 50; i++ {
		got, err := ValidateInputStruct(orderedParams{})
		if err == nil {
			t.Fatal("expected a validation error")
		}
		if got != want {
			t.Fatalf("run %d returned %q, want %q", i, got, want)
		}
		if !strings.HasPrefix(got, " Mail") {
			t.Fatalf("run %d returned %q", i, got)
		}
	}
}
