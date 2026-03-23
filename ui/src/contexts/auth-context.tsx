import {
  createContext,
  type ReactNode,
  useContext,
  useEffect,
  useState,
} from "react";
import { clearToken, setToken, type UserPublic } from "@/lib/api";

interface AuthContextValue {
  user: UserPublic | null;
  token: string | null;
  login: (token: string, user: UserPublic) => void;
  logout: () => void;
  isAuthenticated: boolean;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<UserPublic | null>(() => {
    const stored = localStorage.getItem("cms_user");
    return stored ? JSON.parse(stored) : null;
  });

  const [token, setTokenState] = useState<string | null>(() => {
    return localStorage.getItem("cms_token");
  });

  const handleLogin = (newToken: string, newUser: UserPublic) => {
    setToken(newToken);
    localStorage.setItem("cms_user", JSON.stringify(newUser));
    setTokenState(newToken);
    setUser(newUser);
  };

  const handleLogout = () => {
    clearToken();
    setTokenState(null);
    setUser(null);
  };

  useEffect(() => {
    if (!token) {
      setUser(null);
    }
  }, [token]);

  return (
    <AuthContext.Provider
      value={{
        user,
        token,
        login: handleLogin,
        logout: handleLogout,
        isAuthenticated: !!token,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth must be used within AuthProvider");
  return ctx;
}
