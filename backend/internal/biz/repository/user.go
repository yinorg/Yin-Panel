package repository

import (
	"errors"

	"gorm.io/gorm"
)

type User struct {
	BaseModel
	Password     string `gorm:"type:varchar(255)" json:"password"`  // 密码
	Name         string `gorm:"type:varchar(20)" json:"name"`       // 名称
	HeadImage    string `gorm:"type:varchar(255)" json:"headImage"` // 头像地址
	Status       int8   `gorm:"type:tinyint" json:"status"`         // 状态 1.启用 2.停用 3.未激活
	Role         int8   `gorm:"type:tinyint" json:"role"`           // 角色 1.管理员 2.普通用户
	Mail         string `gorm:"type:varchar(255)" json:"mail"`      // 邮箱
	Token        string `gorm:"-" json:"token"`                     // 仅用于API返回
	TokenVersion uint   `gorm:"not null;default:0" json:"-"`        // 用于撤销已签发的JWT
}

func (r *UserRepo) GetByMail(mail string) (User, error) {
	var user User
	err := Db.Where("mail=?", mail).First(&user).Error
	return user, err
}

type UserRepo struct {
}

type IUserRepo interface {
	Get(id uint) (User, error)
	Count() (uint, error)
	GetByMailAndPassword(mail, password string) (User, error)
	GetByMail(mail string) (User, error)
	GetList(pagedParam PagedParam) ([]User, uint, error)
	Update(id uint, user *User) error
	UpdateUserInfo(id uint, updateInfo map[string]any) error
	Create(user *User) error
	Delete(userId uint) ([]string, error)
	InvalidateTokens(userID uint) error
}

func NewUserRepo() IUserRepo {
	return &UserRepo{}
}

func (r *UserRepo) Get(id uint) (User, error) {
	mUser := User{}
	err := Db.Where("id=?", id).First(&mUser).Error
	return mUser, err
}

func (r *UserRepo) Count() (uint, error) {
	var count int64
	err := Db.Model(&User{}).Count(&count).Error
	return uint(count), err
}

func (r *UserRepo) GetByMailAndPassword(mail, password string) (User, error) {
	user := User{}
	err := Db.Where("mail=?", mail).Where("password=?", password).First(&user).Error
	return user, err
}

func (r *UserRepo) GetList(pagedParam PagedParam) ([]User, uint, error) {
	var count int64
	if err := Db.Model(&User{}).Count(&count).Error; err != nil {
		return nil, 0, err
	}

	var list []User
	if err := Db.Omit("Password").Limit(pagedParam.Limit).Offset(CalcOffset(pagedParam)).Find(&list).Error; err != nil {
		return nil, 0, err
	}

	return list, uint(count), nil
}

func (r *UserRepo) Update(id uint, user *User) error {
	return Db.Transaction(func(tx *gorm.DB) error {
		var existing User
		if err := tx.First(&existing, id).Error; err != nil {
			return err
		}
		if err := tx.Model(&User{}).Where("id = ?", id).Updates(user).Error; err != nil {
			return err
		}
		if user.Name != existing.Name {
			return updatePersonalSpaceNames(tx, id, user.Name)
		}
		return nil
	})
}

func (r *UserRepo) UpdateUserInfo(userId uint, updateInfo map[string]any) error {
	data := map[string]any{}
	if v, ok := updateInfo["name"]; ok {
		data["name"] = v
	}
	if v, ok := updateInfo["head_image"]; ok {
		data["head_image"] = v
	}
	if v, ok := updateInfo["status"]; ok {
		data["status"] = v
	}
	if v, ok := updateInfo["role"]; ok {
		data["role"] = v
	}

	if v, ok := updateInfo["mail"]; ok {
		hasUser := User{}
		count := Db.Where("mail=?", updateInfo["mail"]).First(&hasUser).RowsAffected
		if count != 0 && hasUser.ID != userId {
			return errors.New("the mail already exists")
		}
		data["mail"] = v
	}
	if v, ok := updateInfo["password"]; ok {
		data["password"] = v
	}

	return Db.Transaction(func(tx *gorm.DB) error {
		var existing User
		if _, hasName := data["name"]; hasName {
			if err := tx.First(&existing, userId).Error; err != nil {
				return err
			}
		}
		if err := tx.Model(&User{}).Where("id = ?", userId).Updates(data).Error; err != nil {
			return err
		}
		if newName, ok := data["name"].(string); ok && newName != existing.Name {
			return updatePersonalSpaceNames(tx, userId, newName)
		}
		return nil
	})
}

func updatePersonalSpaceNames(tx *gorm.DB, userId uint, name string) error {
	if err := tx.Model(&Space{}).
		Where("type = ? AND owner_user_id = ? AND (side = ? OR side = '' OR side IS NULL)", SpaceTypePersonal, userId, "yin").
		Update("name", name).Error; err != nil {
		return err
	}
	return tx.Model(&Space{}).
		Where("type = ? AND owner_user_id = ? AND side = ?", SpaceTypePersonal, userId, "yang").
		Update("name", name+"-B").Error
}

// Create persists a new account.
func (r *UserRepo) Create(user *User) error {
	return Db.Create(user).Error
}

func (r *UserRepo) Delete(userId uint) ([]string, error) {
	return r.DeleteWithSpaces(userId, false)
}

func (r *UserRepo) DeleteWithSpaces(userId uint, force bool) ([]string, error) {
	var fileNames []string
	err := Db.Transaction(func(tx *gorm.DB) error {
		var owned []Space
		if err := tx.Where("owner_user_id = ?", userId).Find(&owned).Error; err != nil {
			return err
		}
		for _, space := range owned {
			if space.Type == SpaceTypeShared && !force {
				return errors.New("user owns shared spaces; transfer them before deletion")
			}
		}
		spaceIDs := make([]uint, 0, len(owned))
		for _, space := range owned {
			spaceIDs = append(spaceIDs, space.ID)
		}
		if len(spaceIDs) > 0 {
			var paired []Space
			if err := tx.Where("pair_id IN ?", spaceIDs).Find(&paired).Error; err != nil {
				return err
			}
			for _, space := range paired {
				spaceIDs = append(spaceIDs, space.ID)
			}
			if err := tx.Where("space_id IN ?", spaceIDs).Delete(&ItemIcon{}).Error; err != nil {
				return err
			}
			if err := tx.Where("space_id IN ?", spaceIDs).Delete(&ItemIconGroup{}).Error; err != nil {
				return err
			}
			if err := tx.Where("space_id IN ?", spaceIDs).Delete(&SpaceMember{}).Error; err != nil {
				return err
			}
			if err := tx.Where("space_id IN ?", spaceIDs).Delete(&SpaceOIDCGroup{}).Error; err != nil {
				return err
			}
			if err := tx.Where("id IN ?", spaceIDs).Delete(&Space{}).Error; err != nil {
				return err
			}
		}
		if err := tx.Where("user_id = ?", userId).Delete(&OAuthIdentity{}).Error; err != nil {
			return err
		}
		// Get all files of the user before deletion
		var files []File
		if err := tx.Where("user_id = ?", userId).Find(&files).Error; err != nil {
			return err
		}

		// Store file names for later deletion from storage
		fileNames = make([]string, 0, len(files))
		for _, file := range files {
			fileNames = append(fileNames, file.FileName)
		}

		// 删除图标
		if err := tx.Delete(&ItemIcon{}, "user_id=?", userId).Error; err != nil {
			return err
		}

		// 删除分组
		if err := tx.Delete(&ItemIconGroup{}, "user_id = ?", userId).Error; err != nil {
			return err
		}

		// 删除模块配置
		if err := tx.Delete(&ModuleConfig{}, "user_id=?", userId).Error; err != nil {
			return err
		}

		// 删除文件记录，并没有删除资源文件
		if err := tx.Delete(&File{}, "user_id=?", userId).Error; err != nil {
			return err
		}

		if err := tx.Delete(&User{}, userId).Error; err != nil {
			return err
		}

		return nil
	})

	return fileNames, err
}

// InvalidateTokens revokes all sessions for a user, including sessions created
// before a process restart, by advancing the version embedded in JWT claims.
func (r *UserRepo) InvalidateTokens(userID uint) error {
	return Db.Model(&User{}).Where("id=?", userID).
		UpdateColumn("token_version", gorm.Expr("token_version + ?", 1)).Error
}
