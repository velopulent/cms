package utils

import (
	"errors"
	"time"

	"github.com/golang-jwt/jwt/v5"

	"github.com/velopulent/cms/apps/backend-go/internal/domain"
)

type JWTSigner struct {
	secret []byte
}

func NewJWTSigner(secret string) JWTSigner {
	return JWTSigner{secret: []byte(secret)}
}

func (s JWTSigner) Create(userID string, now time.Time) (string, error) {
	claims := jwt.MapClaims{
		"sub": userID,
		"exp": now.UTC().Add(24 * time.Hour).Unix(),
	}

	return jwt.NewWithClaims(jwt.SigningMethodHS256, claims).SignedString(s.secret)
}

func (s JWTSigner) Verify(tokenString string) (domain.Claims, error) {
	parsed, err := jwt.Parse(tokenString, func(token *jwt.Token) (interface{}, error) {
		if token.Method != jwt.SigningMethodHS256 {
			return nil, errors.New("unexpected signing method")
		}
		return s.secret, nil
	}, jwt.WithExpirationRequired())
	if err != nil {
		return domain.Claims{}, err
	}

	claims, ok := parsed.Claims.(jwt.MapClaims)
	if !ok || !parsed.Valid {
		return domain.Claims{}, errors.New("invalid token")
	}

	sub, err := claims.GetSubject()
	if err != nil {
		return domain.Claims{}, err
	}
	exp, err := claims.GetExpirationTime()
	if err != nil {
		return domain.Claims{}, err
	}

	return domain.Claims{Sub: sub, Exp: exp.Unix()}, nil
}
