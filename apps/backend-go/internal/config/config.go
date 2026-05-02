package config

import (
	"fmt"
	"os"
	"strconv"

	"github.com/joho/godotenv"
)

const (
	defaultJWTSecret  = "cms-jwt-secret-change-in-production"
	defaultHMACSecret = "cms-hmac-secret-change-in-production"
)

type Config struct {
	DatabaseURL          string
	JWTSecret            string
	HMACSecret           string
	BindAddress          string
	GRPCBindAddress      string
	StorageFSPath        string
	S3AccessKeyID        string
	S3SecretAccessKey    string
	S3Bucket             string
	S3Region             string
	S3Endpoint           string
	S3PublicURL          string
	MaxUploadSizeBytes   int
	CookieSecure         bool
	DBMaxConnections     int
	DBMinConnections     int
	DBAcquireTimeoutSecs int
	DBIdleTimeoutSecs    int
	RateLimitMaxRequests int
	RateLimitWindowSecs  int
}

func FromEnv() Config {
	_ = godotenv.Load()

	jwtSecret := envString("JWT_SECRET", defaultJWTSecret)
	if jwtSecret == defaultJWTSecret {
		fmt.Fprintln(os.Stderr, "WARNING: Using default JWT secret. Set JWT_SECRET environment variable in production!")
	}

	hmacSecret := envString("HMAC_SECRET", defaultHMACSecret)
	if hmacSecret == defaultHMACSecret {
		fmt.Fprintln(os.Stderr, "WARNING: Using default HMAC secret. Set HMAC_SECRET environment variable in production!")
	}

	return Config{
		DatabaseURL:          envString("DATABASE_URL", "sqlite:cms.db"),
		JWTSecret:            jwtSecret,
		HMACSecret:           hmacSecret,
		BindAddress:          envString("BIND_ADDRESS", "0.0.0.0:3000"),
		GRPCBindAddress:      envString("GRPC_BIND_ADDRESS", "0.0.0.0:50051"),
		StorageFSPath:        os.Getenv("STORAGE_FS_PATH"),
		S3AccessKeyID:        os.Getenv("S3_ACCESS_KEY_ID"),
		S3SecretAccessKey:    os.Getenv("S3_SECRET_ACCESS_KEY"),
		S3Bucket:             os.Getenv("S3_BUCKET"),
		S3Region:             os.Getenv("S3_REGION"),
		S3Endpoint:           os.Getenv("S3_ENDPOINT"),
		S3PublicURL:          os.Getenv("S3_PUBLIC_URL"),
		MaxUploadSizeBytes:   envInt("MAX_UPLOAD_SIZE_MB", 50) * 1024 * 1024,
		CookieSecure:         envBool("COOKIE_SECURE", false),
		DBMaxConnections:     envInt("DB_MAX_CONNECTIONS", 10),
		DBMinConnections:     envInt("DB_MIN_CONNECTIONS", 2),
		DBAcquireTimeoutSecs: envInt("DB_ACQUIRE_TIMEOUT_SECS", 30),
		DBIdleTimeoutSecs:    envInt("DB_IDLE_TIMEOUT_SECS", 600),
		RateLimitMaxRequests: envInt("RATE_LIMIT_MAX_REQUESTS", 100),
		RateLimitWindowSecs:  envInt("RATE_LIMIT_WINDOW_SECS", 60),
	}
}

func (c Config) HasS3() bool {
	return c.S3AccessKeyID != "" && c.S3SecretAccessKey != "" && c.S3Bucket != ""
}

func envString(name, fallback string) string {
	if value := os.Getenv(name); value != "" {
		return value
	}
	return fallback
}

func envInt(name string, fallback int) int {
	value, err := strconv.Atoi(os.Getenv(name))
	if err != nil {
		return fallback
	}
	return value
}

func envBool(name string, fallback bool) bool {
	switch os.Getenv(name) {
	case "true", "1":
		return true
	case "false", "0":
		return false
	default:
		return fallback
	}
}
