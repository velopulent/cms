package repository

import (
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

type FileSystemStorage struct {
	root string
}

func NewFileSystemStorage(root string) (*FileSystemStorage, error) {
	if err := os.MkdirAll(root, 0o755); err != nil {
		return nil, err
	}
	return &FileSystemStorage{root: root}, nil
}

func (s *FileSystemStorage) Put(_ context.Context, key string, r io.Reader, _ string) error {
	path := filepath.Join(s.root, filepath.FromSlash(key))
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	f, err := os.Create(path)
	if err != nil {
		return err
	}
	defer f.Close()
	_, err = io.Copy(f, r)
	return err
}

func (s *FileSystemStorage) Get(_ context.Context, key string) ([]byte, error) {
	return os.ReadFile(filepath.Join(s.root, filepath.FromSlash(key)))
}

func (s *FileSystemStorage) Delete(_ context.Context, key string) error {
	return os.Remove(filepath.Join(s.root, filepath.FromSlash(key)))
}

func (s *FileSystemStorage) URL(_ string, fileID string) string { return "/api/files/" + fileID }
func (s *FileSystemStorage) Name() string { return "filesystem" }

type S3Storage struct { publicURL string }
func NewS3Storage(publicURL string) *S3Storage { return &S3Storage{publicURL: publicURL} }
func (s *S3Storage) Put(context.Context, string, io.Reader, string) error { return fmt.Errorf("s3 storage is configured but object storage client is not enabled in this build") }
func (s *S3Storage) Get(context.Context, string) ([]byte, error) { return nil, fmt.Errorf("s3 storage is configured but object storage client is not enabled in this build") }
func (s *S3Storage) Delete(context.Context, string) error { return fmt.Errorf("s3 storage is configured but object storage client is not enabled in this build") }
func (s *S3Storage) URL(key, _ string) string { if s.publicURL != "" { return strings.TrimRight(s.publicURL, "/") + "/" + key }; return "/api/files?key=" + key }
func (s *S3Storage) Name() string { return "s3" }
