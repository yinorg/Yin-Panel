package repository

type File struct {
	BaseModel
	UserId   uint   `gorm:"index" json:"userId"`
	FileName string `gorm:"type:varchar(100)" json:"fileName"`
}

type FileRepo struct{}

type IFileRepo interface {
	Get(userId, id uint) (File, error)
	GetList(userId uint) ([]File, uint, error)
	Delete(userId, id uint) error
}

func NewFileRepo() *FileRepo {
	return &FileRepo{}
}

func (r *FileRepo) AddFile(userId uint, fileName string) (File, error) {
	file := File{
		UserId:   userId,
		FileName: fileName,
	}
	err := Db.Create(&file).Error
	return file, err
}

func (r *FileRepo) Get(userId, id uint) (File, error) {
	var file File
	err := Db.Where("user_id=? AND id=?", userId, id).First(&file).Error
	return file, err
}

func (r *FileRepo) GetList(userId uint) ([]File, uint, error) {
	var list []File
	var count int64
	query := Db.Model(&File{}).Where("user_id = ?", userId)
	if err := query.Count(&count).Error; err != nil {
		return nil, 0, err
	}
	err := query.Order("created_at desc").Find(&list).Error
	return list, uint(count), err
}

func (r *FileRepo) GetAll() ([]File, uint, error) {
	var list []File
	var count int64
	query := Db.Model(&File{})
	if err := query.Count(&count).Error; err != nil {
		return nil, 0, err
	}
	err := query.Order("created_at desc").Find(&list).Error
	return list, uint(count), err
}

func (r *FileRepo) GetByID(id uint) (File, error) {
	var file File
	err := Db.Where("id = ?", id).First(&file).Error
	return file, err
}

func (r *FileRepo) Delete(userId, id uint) error {
	return Db.Delete(&File{}, "id = ? AND user_id = ?", id, userId).Error
}

func (r *FileRepo) DeleteByID(id uint) error {
	return Db.Delete(&File{}, "id = ?", id).Error
}
