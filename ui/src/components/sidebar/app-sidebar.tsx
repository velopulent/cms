"use client"

import * as React from "react"
import { useQuery } from "@tanstack/react-query"
import { useParams } from "@tanstack/react-router"
import {
  LayoutDashboard,
  Layers,
  FileText,
  GalleryVerticalEnd,
} from "lucide-react"

import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarRail,
} from "@/components/ui/sidebar"
import { NavMain } from "@/components/sidebar/nav-main"
import { NavProjects } from "@/components/sidebar/nav-projects"
import { NavUser } from "@/components/sidebar/nav-user"
import { TeamSwitcher } from "@/components/sidebar/team-switcher"
import { useAuth } from "@/contexts/auth-context"
import { getSchemas, getSites } from "@/lib/api"

export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  const { siteId } = useParams({ from: "/_admin/sites/$siteId" as any })
  const auth = useAuth()

  const { data: sites } = useQuery({
    queryKey: ["sites"],
    queryFn: getSites,
  })

  const { data: schemas } = useQuery({
    queryKey: ["schemas", siteId],
    queryFn: () => getSchemas(siteId as string),
    enabled: !!siteId,
  })

  const teams = (sites ?? []).map((site) => ({
    name: site.name,
    id: site.id,
    plan: site.role,
    icon: <GalleryVerticalEnd className="size-4" />,
  }))

  const navMain: {
    title: string
    url: string
    icon: React.ReactNode
    isActive?: boolean
    items?: { title: string; url: string }[]
  }[] = [
    {
      title: "Dashboard",
      url: `/sites/${siteId}`,
      icon: <LayoutDashboard />,
      isActive: true,
      items: [
        { title: "Overview", url: `/sites/${siteId}` },
      ],
    },
    {
      title: "Schemas",
      url: `/sites/${siteId}/schemas`,
      icon: <Layers />,
      items: (schemas ?? []).map((s) => ({
        title: s.name,
        url: `/sites/${siteId}/schemas`,
      })),
    },
  ]

  const contentNavItems = (schemas ?? []).map((s) => ({
    title: s.name,
    url: `/sites/${siteId}/content/${s.slug}`,
    icon: <FileText />,
  }))

  if (contentNavItems.length > 0) {
    navMain.push({
      title: "Content",
      url: "#",
      icon: <FileText />,
      isActive: false,
      items: contentNavItems,
    })
  }

  const navUser = {
    name: auth.user?.username ?? "User",
    email: auth.user?.email ?? "",
    avatar: "",
  }

  return (
    <Sidebar collapsible="icon" {...props}>
      <SidebarHeader>
        <TeamSwitcher teams={teams} />
      </SidebarHeader>
      <SidebarContent>
        <NavMain items={navMain} />
        <NavProjects projects={[]} />
      </SidebarContent>
      <SidebarFooter>
        <NavUser user={navUser} />
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  )
}
