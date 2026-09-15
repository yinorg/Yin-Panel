package storage

import (
	"fmt"
	"strings"
)

// ContentAddressedKey returns the canonical private object key for a SHA-256
// digest. The two directory levels avoid large flat object namespaces.
func ContentAddressedKey(hexDigest, extension string) (string, error) {
	digest := strings.ToLower(strings.TrimSpace(hexDigest))
	if len(digest) != 64 {
		return "", fmt.Errorf("SHA-256 digest must contain 64 hexadecimal characters")
	}
	for _, char := range digest {
		if !((char >= '0' && char <= '9') || (char >= 'a' && char <= 'f')) {
			return "", fmt.Errorf("SHA-256 digest must be hexadecimal")
		}
	}
	extension = strings.ToLower(strings.TrimSpace(extension))
	if extension != "" && !strings.HasPrefix(extension, ".") {
		extension = "." + extension
	}
	if strings.ContainsAny(extension, "/\\") {
		return "", fmt.Errorf("invalid file extension")
	}
	return digest[:2] + "/" + digest[2:4] + "/" + digest + extension, nil
}
