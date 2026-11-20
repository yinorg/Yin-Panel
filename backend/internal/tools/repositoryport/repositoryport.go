// Package repositoryport exposes the repository methods as free functions for
// `rust-anything bridge`.
//
// This package is not part of the product: nothing in the server imports it.
// The repository types hold a package-level `*gorm.DB`, so the bridge sees only
// their constructors; these functions open that handle for one call and let the
// bridge record and diff the real methods against a database file.
package repositoryport

import (
	"errors"
	"fmt"
	"os"

	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/schema"
)

// open points the package-level handle at `dbPath` for the duration of one call.
func open(dbPath string) (*gorm.DB, error) {
	if dbPath == "" {
		return nil, errors.New("database path is required")
	}
	if !fileExists(dbPath) {
		return nil, fmt.Errorf("database not found: %s", dbPath)
	}
	db, err := gorm.Open(sqlite.Open(dbPath), &gorm.Config{
		NamingStrategy: schema.NamingStrategy{SingularTable: true},
	})
	if err != nil {
		return nil, err
	}
	repository.Db = db
	return db, nil
}

func fileExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

// CountUsers is `UserRepo.Count` against `dbPath`.
func CountUsers(dbPath string) (uint, error) {
	if _, err := open(dbPath); err != nil {
		return 0, err
	}
	return (&repository.UserRepo{}).Count()
}

// GetUserByMail is `UserRepo.GetByMail` against `dbPath`.
func GetUserByMail(dbPath, mail string) (repository.User, error) {
	if _, err := open(dbPath); err != nil {
		return repository.User{}, err
	}
	return (&repository.UserRepo{}).GetByMail(mail)
}

// UserMailAndName is the pair the repo reads, without the embedded model.
func UserMailAndName(dbPath, mail string) (string, string, error) {
	user, err := GetUserByMail(dbPath, mail)
	if err != nil {
		return "", "", err
	}
	return user.Mail, user.Name, nil
}

// UpdateUserName is `UserRepo.UpdateUserInfo` restricted to the nickname.
func UpdateUserName(dbPath string, userID uint, name string) error {
	if _, err := open(dbPath); err != nil {
		return err
	}
	return (&repository.UserRepo{}).UpdateUserInfo(userID, map[string]any{"name": name})
}

// ListGroups is `ItemIconGroupRepo.GetList` for one user.
func ListGroups(dbPath string, userID uint) ([]repository.ItemIconGroup, error) {
	if _, err := open(dbPath); err != nil {
		return nil, err
	}
	return (&repository.ItemIconGroupRepo{}).GetList(userID)
}

// CountGroups is `ItemIconGroupRepo.Count` for one user.
func CountGroups(dbPath string, userID uint) (int, error) {
	if _, err := open(dbPath); err != nil {
		return 0, err
	}
	return (&repository.ItemIconGroupRepo{}).Count(userID)
}

// GetUserPassword is the hash `UserRepo.GetByMail` returns, as a string.
func GetUserPassword(dbPath, mail string) (string, error) {
	user, err := GetUserByMail(dbPath, mail)
	if err != nil {
		return "", err
	}
	return user.Password, nil
}

// PasswordMatches is `UserRepo.GetByMailAndPassword`: the row exists only when
// both values match.
func PasswordMatches(dbPath, mail, password string) (bool, error) {
	if _, err := open(dbPath); err != nil {
		return false, err
	}
	_, err := (&repository.UserRepo{}).GetByMailAndPassword(mail, password)
	if err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return false, nil
		}
		return false, err
	}
	return true, nil
}

// GroupTitles is `ItemIconGroupRepo.GetList`, reduced to the titles.
func GroupTitles(dbPath string, userID uint) ([]string, error) {
	groups, err := ListGroups(dbPath, userID)
	if err != nil {
		return nil, err
	}
	titles := make([]string, 0, len(groups))
	for _, group := range groups {
		titles = append(titles, group.Title)
	}
	return titles, nil
}
