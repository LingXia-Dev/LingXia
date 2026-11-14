// Generated from the doc file by scripts/generate.mjs; do not edit.
import type { View } from '@lingxia/view';
import { Specimen as DialogSpecimen } from '../../../core/src/Dialog/Dialog.examples';
import { Specimen as DropdownMenuSpecimen } from '../../../core/src/DropdownMenu/DropdownMenu.examples';
import { Specimen as TooltipSpecimen } from '../../../core/src/Tooltip/Tooltip.examples';
import { Specimen as BottomSheetSpecimen } from '../../../core/src/BottomSheet/BottomSheet.examples';
import { Specimen as PopoverSpecimen } from '../../../core/src/Popover/Popover.examples';
import { Category, GalleryPage, Tile } from '../../shared/Layout';

type Actions = { open(target: { page: string }): Promise<void> };

export function render({ actions }: View<Actions>) {
  return (
    <GalleryPage>
      <text className="text-body text-muted">{"Surfaces that float over the page: dialogs, sheets, menus and popovers."}</text>
      <Category title={"5 components"}>
        <Tile
          name={"Dialog"}
          tagline={"A decision, on top"}
          onOpen={() => actions.open({ page: "dialog" })}
          testId={"open-dialog"}
        >
          <DialogSpecimen />
        </Tile>
        <Tile
          name={"DropdownMenu"}
          tagline={"A menu of actions"}
          onOpen={() => actions.open({ page: "dropdown-menu" })}
          testId={"open-dropdown-menu"}
        >
          <DropdownMenuSpecimen />
        </Tile>
        <Tile
          name={"Tooltip"}
          tagline={"A note on demand"}
          onOpen={() => actions.open({ page: "tooltip" })}
          testId={"open-tooltip"}
        >
          <TooltipSpecimen />
        </Tile>
        <Tile
          name={"BottomSheet"}
          tagline={"In reach of a thumb"}
          onOpen={() => actions.open({ page: "bottom-sheet" })}
          testId={"open-bottom-sheet"}
        >
          <BottomSheetSpecimen />
        </Tile>
        <Tile
          name={"Popover"}
          tagline={"A panel beside it"}
          isWide
          onOpen={() => actions.open({ page: "popover" })}
          testId={"open-popover"}
        >
          <PopoverSpecimen />
        </Tile>
      </Category>
    </GalleryPage>
  );
}
