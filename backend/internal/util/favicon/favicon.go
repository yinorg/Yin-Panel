package favicon

import (
	"context"
	"errors"
	"fmt"
	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/zaplog"
	"github.com/yinorg/Yin-Panel/backend/internal/util"
	"net/http"
	"net/url"
	"path"
	"strconv"
	"strings"
	"time"

	"github.com/PuerkitoBio/goquery"
)

const (
	// A page fetch is discovery only, so it stays well inside the caller's budget.
	faviconPageTimeout = 8 * time.Second
	// The icon itself can be larger, but a stalled host must still free its slot.
	faviconImageTimeout = 10 * time.Second
)

// ErrIconUnavailable separates "this site serves no icon" — an answer the caller
// acts on with a text fallback — from a transport failure, which is a real fault.
var ErrIconUnavailable = errors.New("icon unavailable")

func GetOneFaviconURL(ctx context.Context, urlStr string) (string, error) {
	iconURLs, err := getFaviconURL(ctx, urlStr)
	if err != nil {
		return "", err
	}

	pageURL, err := url.Parse(urlStr)
	if err != nil || pageURL.Scheme == "" || pageURL.Host == "" {
		return "", fmt.Errorf("invalid page URL")
	}

	for _, v := range iconURLs {
		iconURL, err := url.Parse(strings.TrimSpace(v))
		if err != nil || iconURL.IsAbs() && iconURL.Scheme != "http" && iconURL.Scheme != "https" {
			continue
		}
		resolved := pageURL.ResolveReference(iconURL)
		if resolved.Scheme == "http" || resolved.Scheme == "https" {
			return resolved.String(), nil
		}
	}
	return "", ErrIconUnavailable
}

// 下载图片
func DownloadImage(ctx context.Context, url string) (string, error) {
	// 创建请求
	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return "", err
	}

	// 发送请求
	client := &http.Client{Timeout: faviconImageTimeout}
	response, err := client.Do(req)
	if err != nil {
		return "", err
	}
	defer func() {
		if err := response.Body.Close(); err != nil {
			zaplog.Logger.Errorf("failed to close resp.Body. error : %v", err)
		}
	}()

	// 检查HTTP响应状态
	if response.StatusCode != http.StatusOK {
		return "", fmt.Errorf("HTTP request failed, status code: %d", response.StatusCode)
	}

	// 限制最大下载大小为 10MB
	limitedReader := http.MaxBytesReader(nil, response.Body, 10*1024*1024)

	// 生成文件名
	urlFileName := path.Base(url)
	fileExt := path.Ext(url)
	if fileExt == "" {
		fileExt = ".ico"
	}
	fileName := util.Md5(fmt.Sprintf("%s%s", urlFileName, time.Now().String())) + fileExt

	// 上传文件
	err = global.Storage.Upload(ctx, limitedReader, fileName)
	if err != nil {
		if strings.Contains(err.Error(), "request body too large") {
			return "", fmt.Errorf("文件太大，不下载")
		}
		return "", fmt.Errorf("failed to upload file: %v", err)
	}

	return fileName, nil
}

func getFaviconURL(ctx context.Context, pageURLString string) ([]string, error) {
	var icons []string
	icons = make([]string, 0)
	client := &http.Client{Timeout: faviconPageTimeout}
	req, err := http.NewRequestWithContext(ctx, "GET", pageURLString, nil)
	if err != nil {
		return icons, err
	}

	// 设置User-Agent头字段，模拟浏览器请求
	req.Header.Set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.3")

	resp, err := client.Do(req)
	if err != nil {
		return icons, err
	}

	defer func() {
		if err := resp.Body.Close(); err != nil {
			zaplog.Logger.Errorf("failed to close resp.Body. error : %v", err)
		}
	}()

	if resp.StatusCode != http.StatusOK {
		return icons, errors.New("HTTP request failed with status code " + strconv.Itoa(resp.StatusCode))
	}

	doc, err := goquery.NewDocumentFromReader(resp.Body)
	if err != nil {
		return icons, err
	}

	// 查找所有link标签，筛选包含rel属性为"icon"的标签
	doc.Find("link").Each(func(i int, s *goquery.Selection) {
		rel, _ := s.Attr("rel")
		href, _ := s.Attr("href")

		if strings.Contains(rel, "icon") && href != "" {
			// fmt.Println(href)
			icons = append(icons, href)
		}
	})

	if len(icons) == 0 {
		// Many sites serve the conventional favicon without declaring it in HTML.
		// Keep this fallback after parsing the page so an explicitly declared icon
		// still takes precedence.
		pageURL, parseErr := url.Parse(pageURLString)
		if parseErr == nil && pageURL.Scheme != "" && pageURL.Host != "" {
			return []string{pageURL.Scheme + "://" + pageURL.Host + "/favicon.ico"}, nil
		}
		return icons, ErrIconUnavailable
	}

	return icons, nil
}
