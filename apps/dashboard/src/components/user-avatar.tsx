import BoringAvatar from "boring-avatars";

import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { cn } from "@/lib/utils";

type UserAvatarProps = {
  username: string;
  image?: string | null;
  className?: string | undefined;
};

export function UserAvatar({ username, image, className }: UserAvatarProps) {
  return (
    <Avatar className={cn(["size-8", className])}>
      <AvatarImage src={image ?? undefined} alt={username} />

      <AvatarFallback className="p-0">
        <BoringAvatar
          size={32}
          className="size-8!"
          name={username}
          variant="beam"
        />
      </AvatarFallback>
    </Avatar>
  );
}
