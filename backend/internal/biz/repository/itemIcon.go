package repository

import (
	"encoding/json"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/param/commonApi"

	"gorm.io/gorm"
)

type ItemIconIconInfo struct {
	ItemType        int    `json:"itemType"`
	Src             string `json:"src"`
	FileName        string `json:"fileName"`
	Text            string `json:"text"`
	BackgroundColor string `json:"backgroundColor"`
}

type ItemIcon struct {
	BaseModel
	IconJson        string           `gorm:"type:text" json:"-"`
	Icon            ItemIconIconInfo `gorm:"-" json:"icon"`
	Title           string           `gorm:"type:varchar(50)" json:"title"`
	Url             string           `gorm:"type:text" json:"url"`
	LanUrl          string           `gorm:"type:text" json:"lanUrl"`
	MobileUrl       string           `gorm:"type:text" json:"mobileUrl"`
	Description     string           `gorm:"type:text" json:"description"`
	OpenMethod      int              `gorm:"type:tinyint(1)" json:"openMethod"`
	Sort            int              `gorm:"type:int(11)" json:"sort"`
	ItemIconGroupId uint             `json:"itemIconGroupId"`
	UserId          uint             `gorm:"index" json:"userId"`
	SpaceID         uint             `json:"spaceId"`
}

type ItemIconRepo struct{}

func NewItemIconRepo() *ItemIconRepo {
	return &ItemIconRepo{}
}

func (itemIconRepo *ItemIconRepo) Get(userId, id uint) (*ItemIcon, error) {
	item := &ItemIcon{}
	err := Db.Where("id = ? AND user_id = ?", id, userId).First(item).Error
	if err != nil {
		return nil, err
	}

	return item, nil
}

func (itemIconRepo *ItemIconRepo) Save(itemIcon *ItemIcon) error {
	if itemIcon.ID != 0 {
		Db.Where("id=?", itemIcon.ID).Updates(&itemIcon)
	} else {
		itemIcon.Sort = 9999
		Db.Create(&itemIcon)
	}
	return nil
}

func (itemIconRepo *ItemIconRepo) GetList(userId, groupId uint) ([]ItemIcon, error) {
	var itemIcons []ItemIcon
	err := Db.Order("sort ,created_at").Find(&itemIcons, "item_icon_group_id = ? AND user_id=?", groupId, userId).Error
	if err != nil {
		return nil, err
	}

	return itemIcons, nil
}

func (itemIconRepo *ItemIconRepo) Delete(userId, id uint) error {
	// Start a transaction to ensure data consistency
	return Db.Transaction(func(tx *gorm.DB) error {
		// Find the item to get its icon path
		var item ItemIcon
		if err := tx.Where("id = ? AND user_id = ?", id, userId).First(&item).Error; err != nil {
			return err
		}

		// Delete associated file
		var icon map[string]any
		if err := json.Unmarshal([]byte(item.IconJson), &icon); err == nil {
			// Check if the icon has a src field indicating a file path
			if fileName, ok := icon["fileName"].(string); ok {
				// Find and delete the file record
				var file File
				if err := tx.Where("file_name = ? AND user_id = ?", fileName, userId).First(&file).Error; err == nil {
					if err := tx.Delete(&file).Error; err != nil {
						return err
					}
				}
			}
		}

		// Delete the item icon
		if err := tx.Delete(&ItemIcon{}, "id = ? AND user_id = ?", id, userId).Error; err != nil {
			return err
		}

		return nil
	})
}

func (itemIconRepo *ItemIconRepo) BatchSaveSort(userId, groupId uint, sortItems []commonApi.SortRequestItem) error {
	return Db.Transaction(func(tx *gorm.DB) error {
		for _, v := range sortItems {
			if err := tx.Model(&ItemIcon{}).Where("user_id=? AND id=? AND item_icon_group_id=?", userId, v.Id, groupId).Update("sort", v.Sort).Error; err != nil {
				return err
			}
		}

		// 返回 nil 提交事务
		return nil
	})
}
