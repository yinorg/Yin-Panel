package repository

type File struct {
	BaseModel
	UserId   uint   `gorm:"index:idx_file_user_name,priority:1" json:"userId"`
	FileName string `gorm:"type:varchar(100);index:idx_file_user_name,priority:2" json:"fileName"`
}

type FileRepo struct{}

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

// GetPaged returns one page of files and the total count. It bounds the rows
// read from the database instead of slicing in the handler.
func (r *FileRepo) GetPaged(pagedParam PagedParam) ([]File, uint, error) {
	var count int64
	if err := Db.Model(&File{}).Count(&count).Error; err != nil {
		return nil, 0, err
	}
	var list []File
	err := Db.Order("created_at desc").Limit(pagedParam.Limit).Offset(CalcOffset(pagedParam)).Find(&list).Error
	return list, uint(count), err
}

func (r *FileRepo) GetByID(id uint) (File, error) {
	var file File
	err := Db.Where("id = ?", id).First(&file).Error
	return file, err
}

func (r *FileRepo) DeleteByID(id uint) error {
	return Db.Delete(&File{}, "id = ?", id).Error
}
