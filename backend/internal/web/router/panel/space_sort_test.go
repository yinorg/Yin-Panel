package panel

import (
	"testing"

	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
)

func TestGroupSortRequiresCompleteUniqueSiblingOrder(t *testing.T) {
	siblings := []repository.ItemIconGroup{
		{BaseModel: repository.BaseModel{ID: 1}},
		{BaseModel: repository.BaseModel{ID: 2}},
		{BaseModel: repository.BaseModel{ID: 3}},
	}
	tests := []struct {
		name    string
		entries []groupSortEntry
		valid   bool
	}{
		{"complete permutation", []groupSortEntry{{ID: 3, Sort: 1}, {ID: 1, Sort: 2}, {ID: 2, Sort: 3}}, true},
		{"missing sibling", []groupSortEntry{{ID: 1, Sort: 1}, {ID: 2, Sort: 2}}, false},
		{"foreign group", []groupSortEntry{{ID: 1, Sort: 1}, {ID: 2, Sort: 2}, {ID: 9, Sort: 3}}, false},
		{"duplicate group", []groupSortEntry{{ID: 1, Sort: 1}, {ID: 1, Sort: 2}, {ID: 3, Sort: 3}}, false},
		{"duplicate sort", []groupSortEntry{{ID: 1, Sort: 1}, {ID: 2, Sort: 1}, {ID: 3, Sort: 3}}, false},
		{"nonpositive sort", []groupSortEntry{{ID: 1, Sort: 0}, {ID: 2, Sort: 2}, {ID: 3, Sort: 3}}, false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := isGroupSortPermutation(siblings, tt.entries); got != tt.valid {
				t.Fatalf("isGroupSortPermutation() = %v, want %v", got, tt.valid)
			}
		})
	}
}
