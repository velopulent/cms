import { createFileRoute, Outlet } from "@tanstack/react-router";
import { DashboardHeader } from "@/components/dashboard-header";

export const Route = createFileRoute("/_admin/_shell")({
  component: ShellLayout,
});

function ShellLayout() {
  return (
    <div className="flex min-h-svh flex-col">
      <DashboardHeader />
      <Outlet />
    </div>
  );
}
