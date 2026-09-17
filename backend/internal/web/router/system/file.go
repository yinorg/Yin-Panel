package system

import (
	"crypto/md5"
	"encoding/hex"
	"github.com/yinorg/Yin-Panel/backend/internal/constant"
	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/zaplog"
	"github.com/yinorg/Yin-Panel/backend/internal/util"
	"github.com/yinorg/Yin-Panel/backend/internal/web/interceptor"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/base"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/response"
	"io"
	"net/http"
	"path"
	"strings"

	"github.com/gin-gonic/gin"
	"github.com/gin-gonic/gin/binding"
)

type FileRouter struct {
	urlPrefix string
}

func NewFileRouter() *FileRouter {
	return &FileRouter{
		urlPrefix: config.AppConfig.Base.URLPrefix,
	}
}

func (a *FileRouter) InitRouter(router *gin.RouterGroup) {
	// 公开接口
	router.GET("/file/s3/*filepath", a.GetS3File)
	r := router.Group("")
	r.Use(interceptor.Auth)
	{
		r.POST("/file/uploadImg", a.UploadImg)
		r.POST("/file/delete", a.Delete)
		r.GET("/file/getList", a.GetList)
	}
}

func (a *FileRouter) InitPublicRouter(router *gin.RouterGroup) {
	router.GET(a.urlPrefix+"*filepath", a.GetS3File)
}

func (a *FileRouter) UploadImg(c *gin.Context) {
	userInfo, exist := base.GetCurrentUserInfo(c)
	if !exist || userInfo.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}

	f, err := c.FormFile("imgfile")
	if err != nil {
		response.ErrorByCode(c, constant.CodeUploadFailed)
		return
	}

	fileExt := strings.ToLower(path.Ext(f.Filename))
	agreeExts := []string{
		".png",
		".jpg",
		".gif",
		".jpeg",
		".webp",
		".svg",
		".ico",
	}

	if !util.InArray(agreeExts, fileExt) {
		response.ErrorByCode(c, constant.CodeUnsupportFileFormat)
		return
	}

	// 打开文件以获取Reader
	src, err := f.Open()
	if err != nil {
		zaplog.Logger.Errorf("Failed to open uploaded file: %v", err)
		response.ErrorByCode(c, constant.CodeUploadFailed)
		return
	}

	defer func() {
		if err := src.Close(); err != nil {
			zaplog.Logger.Errorf("Failed to close file. error : %v", err)
		}
	}()
	hash := md5.New()
	if _, err = io.Copy(hash, src); err != nil {
		response.ErrorByCode(c, constant.CodeUploadFailed)
		return
	}
	fileName := hex.EncodeToString(hash.Sum(nil)) + fileExt
	if _, err = src.Seek(0, io.SeekStart); err != nil {
		response.ErrorByCode(c, constant.CodeUploadFailed)
		return
	}

	// 使用存储接口上传文件
	exists, err := global.Storage.Exists(c.Request.Context(), fileName)
	if err != nil {
		response.ErrorByCode(c, constant.CodeUploadFailed)
		return
	}
	if !exists {
		err = global.Storage.Upload(c.Request.Context(), src, fileName)
		if err != nil {
			zaplog.Logger.Errorf("Failed to upload file: %v", err)
			response.ErrorByCode(c, constant.CodeUploadFailed)
			return
		}
	}

	// 向数据库添加记录
	_, err = global.FileRepo.AddFile(userInfo.ID, fileName)
	if err != nil {
		zaplog.Logger.Errorf("Failed to add file record to database: %v", err)
		response.ErrorByCode(c, constant.CodeUploadFailed)
		return
	}

	filePath := a.urlPrefix + fileName
	zaplog.Logger.Infof("Successfully uploaded file %s to %s", fileName, filePath)
	response.SuccessData(c, gin.H{
		"imageUrl": filePath,
		"fileName": fileName,
	})
}

func (a *FileRouter) GetList(c *gin.Context) {
	userInfo, exist := base.GetCurrentUserInfo(c)
	if !exist || userInfo.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}

	list, count, err := global.FileRepo.GetList(userInfo.ID)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}

	var data []map[string]any
	for _, v := range list {
		data = append(data, map[string]any{
			"src":        a.urlPrefix + v.FileName,
			"fileName":   v.FileName,
			"id":         v.ID,
			"createTime": v.CreatedAt,
			"updateTime": v.UpdatedAt,
		})
	}
	response.SuccessListData(c, data, count)
}

func (a *FileRouter) Delete(c *gin.Context) {
	type RequestDeleteId struct {
		Id uint `json:"id" binding:"required"`
	}

	req := RequestDeleteId{}
	userInfo, exist := base.GetCurrentUserInfo(c)
	if !exist || userInfo.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}

	if err := c.ShouldBindBodyWith(&req, binding.JSON); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}

	file, err := global.FileRepo.Get(userInfo.ID, req.Id)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}

	// 从存储中删除文件
	if err := global.Storage.Delete(c.Request.Context(), file.FileName); err != nil {
		zaplog.Logger.Errorf("Failed to delete file %s: %v", file.FileName, err)
	}

	// 从数据库中删除记录
	if err := global.FileRepo.Delete(userInfo.ID, req.Id); err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}

	response.Success(c)
}

func (a *FileRouter) GetS3File(c *gin.Context) {
	filepath := c.Param("filepath")
	if filepath == "" {
		c.JSON(http.StatusBadRequest, gin.H{"error": "file path is required"})
		return
	}

	// 从存储中读取文件
	fileData, err := global.Storage.Get(c.Request.Context(), filepath)
	if err != nil {
		zaplog.Logger.Errorf("Failed to get file %s: %v", filepath, err)
		c.JSON(http.StatusNotFound, gin.H{"error": "failed to get file"})
		return
	}

	// 设置文件类型
	contentType := "application/octet-stream"
	ext := path.Ext(filepath)
	switch ext {
	case ".jpg", ".jpeg":
		contentType = "image/jpeg"
	case ".png":
		contentType = "image/png"
	case ".gif":
		contentType = "image/gif"
	case ".svg":
		contentType = "image/svg+xml"
	}

	// 设置响应头
	c.Header("Content-Type", contentType)
	c.Header("Content-Disposition", "inline; filename="+path.Base(filepath))
	// Uploaded files use content-hash filenames, so they can be cached indefinitely.
	c.Header("Cache-Control", "public, max-age=31536000, immutable")

	zaplog.Logger.Infof("Successfully serving file: %s with content type: %s", filepath, contentType)
	// 返回文件内容
	c.Data(http.StatusOK, contentType, fileData)
}
