import { Hashvatar } from "hashvatar/react";
import { useState } from "react";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { cn } from "@/lib/utils";

type SiteAvatarProps = {
  siteName: string;
  size?: number;
  siteLogo?: string | null;
  className?: string | undefined;
  animate?: boolean;
};

export function SiteAvatar({
  siteName,
  size = 32,
  siteLogo,
  className,
  animate,
}: SiteAvatarProps) {
  const [isHovered, setIsHovered] = useState(false);
  const shouldAnimate = animate !== undefined ? animate : isHovered;

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
          animated={shouldAnimate}
          className="rounded-lg!"
          dotScale={2.5}
        />
      </AvatarFallback>
    </Avatar>
  );
}
