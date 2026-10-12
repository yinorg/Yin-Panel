package wallpaperpack

import "testing"

// The library is packaged content: it must load, validate against its own files
// and never hand out a path it did not index.
func TestPackCatalogueIsConsistent(t *testing.T) {
	catalogue, err := Catalogue()
	if err != nil {
		t.Fatalf("catalogue: %v", err)
	}
	if catalogue.Version == "" {
		t.Fatal("catalogue has no version; asset URLs would not be cacheable")
	}
	if len(catalogue.Categories) == 0 {
		t.Fatal("catalogue has no categories")
	}
	seen := map[string]bool{}
	total := 0
	for _, category := range catalogue.Categories {
		if category.ID == "" || category.Name == "" {
			t.Fatalf("category %+v is incomplete", category)
		}
		if len(category.Items) == 0 {
			t.Fatalf("category %q has no entries", category.ID)
		}
		for _, item := range category.Items {
			total++
			if seen[item.ID] {
				t.Fatalf("duplicate entry id %q", item.ID)
			}
			seen[item.ID] = true
			if err := valid(item, assets); err != nil {
				t.Fatalf("entry %q is not valid: %v", item.ID, err)
			}
			content, mediaType, ok := Asset(item.Source)
			if !ok || len(content) == 0 {
				t.Fatalf("entry %q source %q is not packaged", item.ID, item.Source)
			}
			if item.Kind == "page" && mediaType != "text/html" {
				t.Fatalf("entry %q page served as %q", item.ID, mediaType)
			}
			if item.Kind == "image" && mediaType[:6] != "image/" {
				t.Fatalf("entry %q image served as %q", item.ID, mediaType)
			}
		}
	}
	if total < 100 {
		t.Fatalf("library holds only %d entries; the pack looks truncated", total)
	}
}

func TestPackAssetRefusesUnindexedNames(t *testing.T) {
	for _, name := range []string{"", "/", "../assets/catalogue.json", "..%2fescape.jpg", "catalogue.json", "assets/catalogue.json", "missing.jpg", `..\windows.jpg`} {
		if content, _, ok := Asset(name); ok {
			t.Fatalf("Asset(%q) returned %d bytes; want refusal", name, len(content))
		}
	}
}

func TestPackFindOnlyReturnsPackagedEntries(t *testing.T) {
	catalogue, err := Catalogue()
	if err != nil {
		t.Fatalf("catalogue: %v", err)
	}
	first := catalogue.Categories[0].Items[0]
	if entry, ok := Find(first.ID); !ok || entry.ID != first.ID {
		t.Fatalf("Find(%q) = %+v, %v; want the packaged entry", first.ID, entry, ok)
	}
	for _, id := range []string{"", "nope-999", "../../etc/passwd"} {
		if entry, ok := Find(id); ok {
			t.Fatalf("Find(%q) = %+v; want refusal", id, entry)
		}
	}
}
