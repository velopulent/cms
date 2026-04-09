"use client";

import { Link, useRouterState } from "@tanstack/react-router";
import { Square } from "lucide-react";
import {
  SidebarGroup,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSkeleton,
} from "@/components/ui/sidebar";

export function NavSingletons({
  singletons,
  isLoading,
}: {
  singletons: {
    name: string;
    slug: string;
    url: string;
  }[];
  isLoading?: boolean;
}) {
  const pathname = useRouterState({ select: (s) => s.location.pathname });

  if (!isLoading && singletons.length === 0) {
    return null;
  }

  return (
    <SidebarGroup className="group-data-[collapsible=icon]:hidden">
      <SidebarGroupLabel>Singletons</SidebarGroupLabel>
      <SidebarMenu>
        {isLoading && singletons.length === 0
          ? [1, 2].map((n) => (
              <SidebarMenuSkeleton
                key={`skeleton-${n}`}
                style={{ opacity: 1 - (n - 1) * 0.2 }}
              />
            ))
          : singletons.map((item) => (
              <SidebarMenuItem key={item.slug}>
                <SidebarMenuButton
                  isActive={pathname === item.url}
                  render={<Link to={item.url} className="my-0.5" />}
                >
                  <Square className="size-4" />
                  <span>{item.name}</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            ))}
      </SidebarMenu>
    </SidebarGroup>
  );
}
