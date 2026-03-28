"use client";

import { useQuery } from "@tanstack/react-query";
import { Link, useParams, useRouterState } from "@tanstack/react-router";
import {
  FileText,
  GalleryVerticalEnd,
  ImageIcon,
  Layers,
  LayoutDashboard,
  Settings,
} from "lucide-react";
import type * as React from "react";
import { NavMain } from "@/components/sidebar/nav-main";
import { NavProjects } from "@/components/sidebar/nav-projects";
import { NavUser } from "@/components/sidebar/nav-user";
import { TeamSwitcher } from "@/components/sidebar/team-switcher";
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

export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  const { siteId } = useParams({ from: "/_admin/sites/$siteId" as any });
  const auth = useAuth();
  const pathname = useRouterState({ select: (s) => s.location.pathname });

  const { data: sites } = useQuery({
    queryKey: ["sites"],
    queryFn: getSites,
  });

  const { data: collections } = useQuery({
    queryKey: ["collections", siteId],
    queryFn: () => getCollections(siteId as string),
    enabled: !!siteId,
  });

  const teams = (sites ?? []).map((site) => ({
    name: site.name,
    id: site.id,
    plan: site.role,
    icon: <GalleryVerticalEnd className="size-4" />,
  }));

  const navMain = [
    {
      title: "Dashboard",
      url: `/sites/${siteId}`,
      icon: <LayoutDashboard />,
    },
    {
      title: "Collections",
      url: `/sites/${siteId}/collections`,
      icon: <Layers />,
    },
    {
      title: "Media Library",
      url: `/sites/${siteId}/media`,
      icon: <ImageIcon />,
    },
  ];

  const contentNavItems = (collections ?? []).map((c) => ({
    name: c.name,
    url: `/sites/${siteId}/content/${c.slug}`,
    icon: <FileText className="size-4" />,
  }));

  const settingsUrl = `/sites/${siteId}/settings`;

  const navUser = {
    name: auth.user?.username ?? "User",
    email: auth.user?.email ?? "",
    avatar: "",
  };

  return (
    <Sidebar collapsible="icon" {...props}>
      <SidebarHeader>
        <TeamSwitcher teams={teams} />
      </SidebarHeader>
      <SidebarContent>
        <NavMain items={navMain} />
        <NavProjects projects={contentNavItems} />
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
