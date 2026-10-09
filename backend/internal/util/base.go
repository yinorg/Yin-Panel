package util

import (
	"crypto/md5"
	"encoding/hex"
	"encoding/json"
	"os"
	"slices"
	"sort"
)

func Md5(str string) string {
	md5Byte := md5.Sum([]byte(str))
	return hex.EncodeToString(md5Byte[:])
}

func PathExists(path string) (bool, error) {
	_, err := os.Stat(path)
	if err == nil {
		return true, nil
	}
	if os.IsNotExist(err) {
		return false, nil
	}
	return false, err
}

func InArray[T uint | int | int8 | int64 | float32 | float64 | string](arr []T, item T) bool {
	slices.Sort(arr)

	index := sort.Search(len(arr), func(i int) bool {
		return arr[i] >= item
	})

	return index < len(arr) && arr[index] == item
}

func PasswordEncryption(password string) string {
	return Md5(Md5(Md5(password)))
}

// ToJSONString toJSON 将对象转换为JSON字符串，如果出错则返回"{}"
func ToJSONString(v any) string {
	if v == nil {
		return "{}"
	}
	b, err := json.Marshal(v)
	if err != nil {
		return "{}"
	}
	return string(b)
}

func RedirectURL(rootUrl, provider string) string {
	return rootUrl + "/api/oauth/" + provider + "/callback"
}
