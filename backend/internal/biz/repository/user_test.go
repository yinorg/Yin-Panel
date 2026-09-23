package repository

import (
	"path/filepath"
	"testing"

	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
)

func TestUserRenameSynchronizesPersonalSpaces(t *testing.T) {
	db, err := gorm.Open(sqlite.Open(filepath.Join(t.TempDir(), "user-repository.db")), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := db.AutoMigrate(&User{}, &Space{}); err != nil {
		t.Fatal(err)
	}
	previousDB := Db
	Db = db
	t.Cleanup(func() { Db = previousDB })

	user := User{Name: "Old Nickname", Mail: "user@example.com", Publiccode: "user-code"}
	if err := db.Create(&user).Error; err != nil {
		t.Fatal(err)
	}
	otherUser := User{Name: "Other", Mail: "other@example.com", Publiccode: "other-code"}
	if err := db.Create(&otherUser).Error; err != nil {
		t.Fatal(err)
	}

	yin := Space{Type: SpaceTypePersonal, Name: user.Name, OwnerUserID: user.ID, Side: "yin"}
	if err := db.Create(&yin).Error; err != nil {
		t.Fatal(err)
	}
	yang := Space{Type: SpaceTypePersonal, Name: user.Name + "-B", OwnerUserID: user.ID, PairID: yin.ID, Side: "yang"}
	if err := db.Create(&yang).Error; err != nil {
		t.Fatal(err)
	}
	shared := Space{Type: SpaceTypeShared, Name: "Shared", OwnerUserID: user.ID, Side: "yin"}
	if err := db.Create(&shared).Error; err != nil {
		t.Fatal(err)
	}
	otherPersonal := Space{Type: SpaceTypePersonal, Name: otherUser.Name, OwnerUserID: otherUser.ID, Side: "yin"}
	if err := db.Create(&otherPersonal).Error; err != nil {
		t.Fatal(err)
	}

	repo := &UserRepo{}
	if err := repo.UpdateUserInfo(user.ID, map[string]any{"name": "Self Renamed"}); err != nil {
		t.Fatalf("self-service rename failed: %v", err)
	}
	assertSpaceName(t, db, yin.ID, "Self Renamed")
	assertSpaceName(t, db, yang.ID, "Self Renamed-B")
	assertSpaceName(t, db, shared.ID, "Shared")
	assertSpaceName(t, db, otherPersonal.ID, "Other")

	if err := db.Model(&Space{}).Where("id = ?", yin.ID).Update("name", "Custom Personal Name").Error; err != nil {
		t.Fatal(err)
	}
	if err := repo.UpdateUserInfo(user.ID, map[string]any{"head_image": "avatar.png"}); err != nil {
		t.Fatalf("profile update without rename failed: %v", err)
	}
	assertSpaceName(t, db, yin.ID, "Custom Personal Name")

	user.Name = "Admin Renamed"
	if err := repo.Update(user.ID, &user); err != nil {
		t.Fatalf("admin rename failed: %v", err)
	}
	assertSpaceName(t, db, yin.ID, "Admin Renamed")
	assertSpaceName(t, db, yang.ID, "Admin Renamed-B")

	otherUser.Name = "Other Renamed"
	if err := repo.UpdateUserInfo(otherUser.ID, map[string]any{"name": otherUser.Name}); err != nil {
		t.Fatalf("rename without paired Yang space failed: %v", err)
	}
	assertSpaceName(t, db, otherPersonal.ID, "Other Renamed")
}

func assertSpaceName(t *testing.T, db *gorm.DB, id uint, want string) {
	t.Helper()
	var space Space
	if err := db.First(&space, id).Error; err != nil {
		t.Fatal(err)
	}
	if space.Name != want {
		t.Fatalf("space %d name = %q, want %q", id, space.Name, want)
	}
}
