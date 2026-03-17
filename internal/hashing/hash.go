package hashing

import (
	"crypto/sha256"
	"encoding/hex"
	"io"
	"os"
	"path/filepath"
	"sort"
)

func Dir(path string) (string, error) {
	var files []string
	err := filepath.Walk(path, func(p string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if info.Mode().IsRegular() {
			files = append(files, p)
		}
		return nil
	})
	if err != nil {
		return "", err
	}
	sort.Strings(files)
	h := sha256.New()
	for _, f := range files {
		rel, _ := filepath.Rel(path, f)
		_, _ = h.Write([]byte(rel))
		_, _ = h.Write([]byte{0})
		fd, err := os.Open(f)
		if err != nil {
			return "", err
		}
		if _, err := io.Copy(h, fd); err != nil {
			_ = fd.Close()
			return "", err
		}
		_ = fd.Close()
		_, _ = h.Write([]byte{0})
	}
	return hex.EncodeToString(h.Sum(nil)), nil
}
