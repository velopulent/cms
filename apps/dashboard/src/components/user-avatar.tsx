import BoringAvatar from "boring-avatars";

export function UserAvatar({
  username,
  size = 32,
  className,
}: {
  username: string;
  size?: number;
  className?: string;
}) {
  return (
    <BoringAvatar
      size={size}
      name={username}
      variant="beam"
      className={className}
    />
  );
}
