"use client";

import { useQuery } from "@tanstack/react-query";
import { Link, useParams, useRouterState } from "@tanstack/react-router";
import {
  Files,
  FileText,
  GalleryVerticalEnd,
  Home,
  Layers,
  Settings,
} from "lucide-react";
import { type ComponentProps, useMemo } from "react";
import { NavCollections } from "@/components/sidebar/nav-collections";
import { NavMain } from "@/components/sidebar/nav-main";
import { NavSingletons } from "@/components/sidebar/nav-singletons";
import { NavUser } from "@/components/sidebar/nav-user";
import { SiteSwitcher } from "@/components/sidebar/site-switcher";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
} from "@/components/ui/sidebar";
import { useAuth } from "@/contexts/auth-context";
import { getCollections, getSites } from "@/lib/api";

export function AppSidebar({ ...props }: ComponentProps<typeof Sidebar>) {
  const { siteId } = useParams({ from: "/_admin/sites/$siteId" as any });
  const auth = useAuth();
  const pathname = useRouterState({ select: (s) => s.location.pathname });

  const { data: sites, isLoading: sitesLoading } = useQuery({
    queryKey: ["sites"],
    queryFn: getSites,
  });

  const { data: collections, isLoading: collectionsLoading } = useQuery({
    queryKey: ["collections", siteId],
    queryFn: () => getCollections(siteId as string),
    enabled: !!siteId,
  });

  const teams = useMemo(
    () =>
      (sites ?? []).map((site) => ({
        name: site.name,
        id: site.id,
        plan: site.role,
        icon: <GalleryVerticalEnd className="size-4" />,
      })),
    [sites],
  );

  const navMain = useMemo(
    () => [
      {
        title: "Home",
        url: `/sites/${siteId}`,
        icon: <Home />,
      },
      {
        title: "Collections",
        url: `/sites/${siteId}/collections`,
        icon: <Layers />,
      },
      {
        title: "Files",
        url: `/sites/${siteId}/files`,
        icon: <Files />,
      },
    ],
    [siteId],
  );

  const contentNavItems = useMemo(
    () =>
      (collections ?? [])
        .filter((c) => !c.is_singleton)
        .map((c) => ({
          name: c.name,
          url: `/sites/${siteId}/entries/${c.slug}`,
          icon: <FileText className="size-4" />,
        })),
    [collections, siteId],
  );

  const singletonNavItems = useMemo(
    () =>
      (collections ?? [])
        .filter((c) => c.is_singleton)
        .map((c) => ({
          name: c.name,
          slug: c.slug,
          url: `/sites/${siteId}/singletons/${c.slug}`,
        })),
    [collections, siteId],
  );

  const settingsUrl = `/sites/${siteId}/settings`;

  const navUser = {
    name: auth.user?.username ?? "User",
    email: auth.user?.email ?? "",
    avatar: "",
  };

  return (
    <Sidebar collapsible="icon" {...props}>
      <SidebarHeader>
        <SiteSwitcher sites={teams} isLoading={sitesLoading} />
      </SidebarHeader>
      <SidebarContent>
        <NavMain items={navMain} />
        <NavSingletons
          singletons={singletonNavItems}
          isLoading={collectionsLoading}
        />
        <NavCollections
          collections={contentNavItems}
          isLoading={collectionsLoading}
        />
      </SidebarContent>
      <SidebarFooter>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              tooltip="Settings"
              isActive={pathname === settingsUrl}
              render={<Link to={settingsUrl} />}
            >
              <Settings />
              <span>Settings</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
        <NavUser user={navUser} />
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  );
}
