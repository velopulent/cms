import { createFileRoute, Outlet } from "@tanstack/react-router";

export const Route = createFileRoute("/_admin/content/$typeSlug")({
  component: () => <Outlet />,
});
