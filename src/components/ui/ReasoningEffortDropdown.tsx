import React, { useEffect, useState } from "react";
import { commands } from "@/bindings";
import { Dropdown, DropdownOption } from "./Dropdown";

interface ReasoningEffortDropdownProps {
  className?: string;
  disabled?: boolean;
  direction?: "up" | "down";
}

const REASONING_EFFORT_OPTIONS: DropdownOption[] = [
  { value: "none", label: "None" },
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
  { value: "xhigh", label: "Extra High" },
];

export const ReasoningEffortDropdown: React.FC<
  ReasoningEffortDropdownProps
> = ({ className = "", disabled = false, direction = "down" }) => {
  const [effort, setEffort] = useState<string>("medium");
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const loadEffort = async () => {
      try {
        const currentEffort = await commands.getOpenaiReasoningEffort();
        setEffort(currentEffort);
      } catch (error) {
        console.error("Failed to load reasoning effort:", error);
      } finally {
        setIsLoading(false);
      }
    };
    loadEffort();
  }, []);

  const handleSelect = async (value: string) => {
    setEffort(value);
    try {
      const result = await commands.setOpenaiReasoningEffort(value);
      if (result.status === "error") {
        console.error("Failed to set reasoning effort:", result.error);
        // Revert on error
        const currentEffort = await commands.getOpenaiReasoningEffort();
        setEffort(currentEffort);
      }
    } catch (error) {
      console.error("Failed to set reasoning effort:", error);
    }
  };

  return (
    <Dropdown
      selectedValue={effort}
      options={REASONING_EFFORT_OPTIONS}
      onSelect={handleSelect}
      disabled={disabled || isLoading}
      placeholder={isLoading ? "Loading..." : "Select effort"}
      className={className}
      direction={direction}
    />
  );
};
