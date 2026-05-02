package httpadapter

import (
	"errors"
	"net/http"
	"strings"

	"github.com/labstack/echo/v4"

	"github.com/velopulent/cms/apps/backend-go/internal/application"
)

type errorBody struct {
	Error string `json:"error"`
}

func writeAppError(c echo.Context, err error) error {
	var appErr application.AppError
	if !errors.As(err, &appErr) {
		return c.JSON(http.StatusInternalServerError, errorBody{Error: err.Error()})
	}

	switch appErr.Kind {
	case application.KindValidation:
		return c.JSON(http.StatusBadRequest, errorBody{Error: appErr.Error()})
	case application.KindConflict:
		return c.JSON(http.StatusConflict, errorBody{Error: appErr.Error()})
	case application.KindUnauthorized:
		return c.JSON(http.StatusUnauthorized, errorBody{Error: "Invalid username or password"})
	case application.KindNotFound:
		return c.JSON(http.StatusNotFound, errorBody{Error: appErr.Error()})
	default:
		return c.JSON(http.StatusInternalServerError, errorBody{Error: appErr.Error()})
	}
}

func writeAnyError(c echo.Context, err error, fallbackStatus int, fallback string) error {
	if err == nil {
		return nil
	}
	var appErr application.AppError
	if errors.As(err, &appErr) {
		switch appErr.Kind {
		case application.KindValidation:
			status := http.StatusBadRequest
			if strings.HasPrefix(appErr.Error(), "File too large") {
				status = http.StatusRequestEntityTooLarge
			}
			return c.JSON(status, errorBody{Error: appErr.Error()})
		case application.KindConflict:
			return c.JSON(http.StatusConflict, errorBody{Error: appErr.Error()})
		case application.KindUnauthorized:
			return c.JSON(http.StatusUnauthorized, errorBody{Error: appErr.Error()})
		case application.KindNotFound:
			return c.JSON(http.StatusNotFound, errorBody{Error: appErr.Error()})
		default:
			return c.JSON(http.StatusInternalServerError, errorBody{Error: appErr.Error()})
		}
	}
	if fallback == "" { fallback = err.Error() }
	return c.JSON(fallbackStatus, errorBody{Error: fallback})
}
