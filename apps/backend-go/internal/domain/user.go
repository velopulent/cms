package domain

type User struct {
	ID           string `json:"id"`
	Username     string `json:"username"`
	Email        string `json:"email"`
	PasswordHash string `json:"-"`
	CreatedAt    string `json:"created_at"`
	UpdatedAt    string `json:"updated_at"`
}

type UserPublic struct {
	ID       string `json:"id"`
	Username string `json:"username"`
	Email    string `json:"email"`
}

type Claims struct {
	Sub string `json:"sub"`
	Exp int64  `json:"exp"`
}

type AuthResponse struct {
	User UserPublic `json:"user"`
}
