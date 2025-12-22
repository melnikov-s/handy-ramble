import React from "react";

interface PlayIconProps {
  width?: number;
  height?: number;
  color?: string;
  className?: string;
}

const PlayIcon: React.FC<PlayIconProps> = ({
  width = 24,
  height = 24,
  color = "#1e40af",
  className = "",
}) => {
  return (
    <svg
      width={width}
      height={height}
      viewBox="0 0 24 24"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
    >
      <path
        d="M8 5.14v13.72c0 1.08 1.21 1.72 2.11 1.11l10.28-6.86c.79-.53.79-1.69 0-2.22L10.11 4.03C9.21 3.42 8 4.06 8 5.14z"
        fill={color}
      />
    </svg>
  );
};

export default PlayIcon;
