package application

import "errors"

var (
	ErrDuplicateUser       = errors.New("duplicate user")
	ErrInvalidCredentials  = errors.New("invalid credentials")
	ErrUserNotFound        = errors.New("user not found")
	ErrUsernameRequired    = errors.New("Username is required")
	ErrUsernameTooShort    = errors.New("Username must be at least 3 characters")
	ErrPasswordRequired    = errors.New("Password is required")
	ErrPasswordTooShort    = errors.New("Password must be at least 8 characters")
	ErrInvalidEmailAddress = errors.New("Invalid email address")
	ErrNotFound            = errors.New("Resource not found")
	ErrSiteNotFound        = errors.New("Site not found")
	ErrCollectionNotFound  = errors.New("Collection not found")
	ErrEntryNotFound       = errors.New("Entry not found")
	ErrFileNotFound        = errors.New("File not found")
	ErrMemberNotFound      = errors.New("Member not found")
	ErrTokenNotFound       = errors.New("Token not found")
	ErrSingletonNotFound   = errors.New("Singleton not found")
)

type Kind string

const (
	KindValidation         Kind = "validation"
	KindConflict           Kind = "conflict"
	KindUnauthorized       Kind = "unauthorized"
	KindNotFound           Kind = "not_found"
	KindInternal           Kind = "internal"
	KindRepositoryConflict Kind = "repository_conflict"
)

type AppError struct {
	Kind Kind
	Err  error
}

func (e AppError) Error() string {
	if e.Err == nil {
		return string(e.Kind)
	}
	return e.Err.Error()
}

func (e AppError) Unwrap() error {
	return e.Err
}

func Validation(err error) error {
	return AppError{Kind: KindValidation, Err: err}
}

func Conflict(err error) error {
	return AppError{Kind: KindConflict, Err: err}
}

func Unauthorized(err error) error {
	return AppError{Kind: KindUnauthorized, Err: err}
}

func NotFound(err error) error {
	return AppError{Kind: KindNotFound, Err: err}
}

func Internal(err error) error {
	return AppError{Kind: KindInternal, Err: err}
}

func Message(err error) string {
	if err == nil {
		return ""
	}
	var appErr AppError
	if errors.As(err, &appErr) {
		return appErr.Error()
	}
	return err.Error()
}
