package theme

import (
	"archive/zip"
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"path"
	"strings"
	"time"

	"gorm.io/gorm"
)

const MaxWebWallpaper = 6 << 20

type WebWallpaperRecord struct {
	ID         string `gorm:"primaryKey;size:64"`
	OwnerID    uint   `gorm:"index"`
	HTML       []byte
	Poster     []byte
	PosterType string `gorm:"size:32"`
	CreatedAt  time.Time
}

func ParseWebWallpaper(data []byte) (html, poster []byte, posterType string, err error) {
	if len(data) == 0 || len(data) > MaxWebWallpaper {
		return nil, nil, "", errors.New("web wallpaper package exceeds 6 MiB")
	}
	zr, err := zip.NewReader(bytes.NewReader(data), int64(len(data)))
	if err != nil {
		return nil, nil, "", err
	}
	if len(zr.File) != 2 {
		return nil, nil, "", errors.New("web wallpaper must contain index.html and one poster")
	}
	for _, file := range zr.File {
		if file.FileInfo().IsDir() || path.Clean(file.Name) != file.Name || strings.Contains(file.Name, "/") || strings.Contains(file.Name, "\\") || file.UncompressedSize64 > MaxWebWallpaper {
			return nil, nil, "", fmt.Errorf("unsafe web wallpaper file %q", file.Name)
		}
		reader, openErr := file.Open()
		if openErr != nil {
			return nil, nil, "", openErr
		}
		content, readErr := io.ReadAll(io.LimitReader(reader, MaxWebWallpaper+1))
		closeErr := reader.Close()
		if readErr != nil || closeErr != nil || len(content) > MaxWebWallpaper {
			return nil, nil, "", errors.New("invalid web wallpaper content")
		}
		switch file.Name {
		case "index.html":
			html = content
		case "poster.png":
			poster, posterType = content, "image/png"
		case "poster.jpg":
			poster, posterType = content, "image/jpeg"
		case "poster.webp":
			poster, posterType = content, "image/webp"
		default:
			return nil, nil, "", fmt.Errorf("unexpected web wallpaper file %q", file.Name)
		}
	}
	if len(html)+len(poster) > MaxWebWallpaper || !validWallpaperBytes(ResourceData{MediaType: "text/html", Content: html}) || !validWallpaperBytes(ResourceData{MediaType: posterType, Content: poster}) {
		return nil, nil, "", errors.New("invalid web wallpaper or poster")
	}
	return html, poster, posterType, nil
}

func SaveWebWallpaper(db *gorm.DB, ownerID uint, archive []byte) (WebWallpaperRecord, error) {
	html, poster, posterType, err := ParseWebWallpaper(archive)
	if err != nil {
		return WebWallpaperRecord{}, err
	}
	sum := sha256.Sum256(append(append([]byte(fmt.Sprintf("%d:", ownerID)), html...), poster...))
	record := WebWallpaperRecord{ID: hex.EncodeToString(sum[:]), OwnerID: ownerID, HTML: html, Poster: poster, PosterType: posterType}
	if err := db.FirstOrCreate(&record, WebWallpaperRecord{ID: record.ID}).Error; err != nil {
		return WebWallpaperRecord{}, err
	}
	return record, nil
}

func WebWallpaperAsset(db *gorm.DB, id, name string) (string, []byte, error) {
	var record WebWallpaperRecord
	if err := db.First(&record, "id = ?", id).Error; err != nil {
		return "", nil, err
	}
	if name == "index.html" {
		return "text/html", record.HTML, nil
	}
	ext := map[string]string{"image/png": "poster.png", "image/jpeg": "poster.jpg", "image/webp": "poster.webp"}[record.PosterType]
	if name == ext {
		return record.PosterType, record.Poster, nil
	}
	return "", nil, gorm.ErrRecordNotFound
}
