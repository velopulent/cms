import { createFileRoute, Outlet } from "@tanstack/react-router";
import { HomeHeader } from "@/components/home-header";

export const Route = createFileRoute("/_admin/_shell")({
  component: ShellLayout,
});

function ShellLayout() {
  return (
    <div className="flex min-h-svh flex-col">
      <HomeHeader />
      <Outlet />
    </div>
  );
}
