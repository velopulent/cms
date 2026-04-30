import { createFileRoute, Outlet } from "@tanstack/react-router";

export const Route = createFileRoute(
  "/_admin/sites/$siteId/entries/$collectionSlug",
)({
  component: () => <Outlet />,
});
