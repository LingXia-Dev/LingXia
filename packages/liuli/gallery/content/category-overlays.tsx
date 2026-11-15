// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Specimen as DialogSpecimen } from '../../core/src/Dialog/Dialog.examples';
import { Specimen as DropdownMenuSpecimen } from '../../core/src/DropdownMenu/DropdownMenu.examples';
import { Specimen as TooltipSpecimen } from '../../core/src/Tooltip/Tooltip.examples';
import { Specimen as BottomSheetSpecimen } from '../../core/src/BottomSheet/BottomSheet.examples';
import { Specimen as PopoverSpecimen } from '../../core/src/Popover/Popover.examples';
import { Specimen as DrawerSpecimen } from '../../core/src/Drawer/Drawer.examples';
import { Category, Tile } from '../shared/Layout';

export function Content({ onOpen }: { onOpen: (page: string) => void }) {
  return (
    <>
      <text className="text-body text-muted">{"Surfaces that float over the page: dialogs, sheets, menus and popovers."}</text>
      <Category title={"6 components"}>
        <Tile
          name={"Dialog"}
          tagline={"A decision, on top"}
          onOpen={() => onOpen("dialog")}
          testId={"open-dialog"}
        >
          <DialogSpecimen />
        </Tile>
        <Tile
          name={"DropdownMenu"}
          tagline={"A menu of actions"}
          onOpen={() => onOpen("dropdown-menu")}
          testId={"open-dropdown-menu"}
        >
          <DropdownMenuSpecimen />
        </Tile>
        <Tile
          name={"Tooltip"}
          tagline={"A note on demand"}
          onOpen={() => onOpen("tooltip")}
          testId={"open-tooltip"}
        >
          <TooltipSpecimen />
        </Tile>
        <Tile
          name={"BottomSheet"}
          tagline={"In reach of a thumb"}
          onOpen={() => onOpen("bottom-sheet")}
          testId={"open-bottom-sheet"}
        >
          <BottomSheetSpecimen />
        </Tile>
        <Tile
          name={"Popover"}
          tagline={"A panel beside it"}
          onOpen={() => onOpen("popover")}
          testId={"open-popover"}
        >
          <PopoverSpecimen />
        </Tile>
        <Tile
          name={"Drawer"}
          tagline={"A side panel"}
          onOpen={() => onOpen("drawer")}
          testId={"open-drawer"}
        >
          <DrawerSpecimen />
        </Tile>
      </Category>
    </>
  );
}
