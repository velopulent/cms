package utils

import (
	"testing"
	"time"
)

func TestJWTCreateAndVerify(t *testing.T) {
	signer := NewJWTSigner("test-secret")

	now := time.Now().UTC()
	token, err := signer.Create("user-123", now)
	if err != nil {
		t.Fatalf("create token: %v", err)
	}
	if token == "" {
		t.Fatal("token is empty")
	}

	claims, err := signer.Verify(token)
	if err != nil {
		t.Fatalf("verify token: %v", err)
	}
	if claims.Sub != "user-123" {
		t.Fatalf("subject mismatch: %q", claims.Sub)
	}
	if claims.Exp != now.Add(24*time.Hour).Unix() {
		t.Fatalf("expiration mismatch: %d", claims.Exp)
	}
}
