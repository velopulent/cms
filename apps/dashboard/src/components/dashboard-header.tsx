import { Link, useNavigate } from "@tanstack/react-router";
import { LogOut, Settings, User } from "lucide-react";
import { ModeToggle } from "@/components/theme-toggle";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Button, buttonVariants } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useAuth } from "@/contexts/auth-context";
import { isOperator } from "@/lib/api";

export function DashboardHeader() {
  const auth = useAuth();
  const navigate = useNavigate();
  const showInstanceSettings = isOperator(auth.user?.instance_role);
  const name = auth.user?.username ?? "User";
  const email = auth.user?.email ?? "";

  const handleLogout = async () => {
    await auth.logout();
    navigate({ to: "/login" });
  };

  return (
    <header className="sticky top-0 z-20 flex h-16 items-center justify-between border-b bg-background/80 px-4 backdrop-blur sm:px-6 max-w-6xl mx-auto w-full">
      <Link to="/" className="flex items-center gap-2 font-semibold">
        <img
          src="/dashboard/logo192.png"
          alt="Velopulent CMS Logo"
          className="size-8"
        />

        <span className="text-base">CMS</span>
      </Link>

      <div className="flex items-center gap-2">
        <ModeToggle />
        {showInstanceSettings && (
          <Link
            to="/settings"
            title="Instance settings"
            aria-label="Instance settings"
            className={buttonVariants({ variant: "outline", size: "icon" })}
          >
            <Settings className="size-[1.2rem]" />
          </Link>
        )}
        <DropdownMenu>
          <DropdownMenuTrigger
            render={
              <Button variant="ghost" size="icon" className="rounded-full" />
            }
          >
            <Avatar className="size-8">
              <AvatarFallback>{name.slice(0, 2).toUpperCase()}</AvatarFallback>
            </Avatar>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="min-w-56 rounded-lg">
            <DropdownMenuGroup>
              <DropdownMenuLabel className="p-0 font-normal">
                <div className="flex items-center gap-2 px-1 py-1.5 text-left text-sm">
                  <Avatar className="size-8">
                    <AvatarFallback>
                      {name.slice(0, 2).toUpperCase()}
                    </AvatarFallback>
                  </Avatar>
                  <div className="grid flex-1 leading-tight">
                    <span className="truncate font-medium">{name}</span>
                    <span className="truncate text-xs text-muted-foreground">
                      {email}
                    </span>
                  </div>
                </div>
              </DropdownMenuLabel>
            </DropdownMenuGroup>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={() => navigate({ to: "/account" })}>
              <User />
              Account
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={handleLogout}>
              <LogOut />
              Log out
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </header>
  );
}
