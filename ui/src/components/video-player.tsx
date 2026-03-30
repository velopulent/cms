"use client";

import "@videojs/react/video/skin.css";
import { createPlayer, videoFeatures } from "@videojs/react";
import { Video, VideoSkin } from "@videojs/react/video";

const Player = createPlayer({ features: videoFeatures });

interface VideoPlayerProps {
  src: string;
  poster?: string;
  className?: string;
}

export function VideoPlayer({ src, poster, className }: VideoPlayerProps) {
  return (
    <div
      className={className}
      style={{
        "--media-object-fit": "cover",
        "--media-border-radius": "0",
      } as React.CSSProperties}
    >
      <Player.Provider>
        <VideoSkin>
          <Video src={src} poster={poster} playsInline />
        </VideoSkin>
      </Player.Provider>
    </div>
  );
}
