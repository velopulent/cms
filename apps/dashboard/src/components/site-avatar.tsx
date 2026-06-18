import { Hashvatar } from "hashvatar/react";
import { Avatar, AvatarImage, AvatarFallback } from "@/components/ui/avatar";
import { cn } from "@/lib/utils";
import { useState } from "react";

type SiteAvatarProps = {
  siteName: string;
  size?: number;
  siteLogo?: string | null;
  className?: string | undefined;
};

export function SiteAvatar({
  siteName,
  size = 32,
  siteLogo,
  className,
}: SiteAvatarProps) {
  const [isHovered, setIsHovered] = useState(false);

  return (
    <Avatar
      className={cn(["size-8 rounded-lg!", className])}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      <AvatarImage src={siteLogo ?? undefined} alt={siteName} />
      <AvatarFallback className="p-0">
        <Hashvatar
          size={size}
          hash={siteName}
          mode="dither"
          animated={isHovered}
          className="rounded-lg!"
          dotScale={2.5}
        />
      </AvatarFallback>
    </Avatar>
  );
}
