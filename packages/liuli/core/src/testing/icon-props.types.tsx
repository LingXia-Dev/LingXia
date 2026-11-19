// Type test, checked by `tsc`: an icon prop takes a name from the set, never a node or free text, so
// a misspelt name fails to compile instead of drawing nothing or the word.
import { Badge } from '../Badge/Badge';
import { Button } from '../Button/Button';
import { Details } from '../Details/Details';
import { DropdownMenu, DropdownMenuItem, DropdownMenuSubmenu } from '../DropdownMenu/DropdownMenu';
import { IconButton } from '../IconButton/IconButton';
import { Item } from '../Item/Item';
import { Popover } from '../Popover/Popover';
import { TextInput } from '../TextInput/TextInput';
import { Token } from '../Token/Token';

const none = () => {};
const word: string = 'search';
const node = <vector aria-hidden paths={[]} viewBox={[0, 0, 24, 24]} />;

export const named = [
  <Badge label="Paid" startIcon="success" />,
  <Button label="Send" startIcon="send" endIcon="chevron-end" onClick={none} />,
  <Details label="More" startIcon="info"><text>Body</text></Details>,
  <DropdownMenu label="Actions" menu={<><DropdownMenuItem label="Delete" icon="delete" onClick={none} /><DropdownMenuSubmenu label="Move" icon="folder"><DropdownMenuItem label="Inbox" onClick={none} /></DropdownMenuSubmenu></>}><IconButton label="Actions" icon="more" /></DropdownMenu>,
  <IconButton label="Close" icon="close" onClick={none} />,
  <Item label="Inbox" startIcon="mail" />,
  <Popover label="Filters" content={<text>Panel</text>}><Button label="Filters" startIcon="filter" /></Popover>,
  <TextInput label="Name" value="" onChange={none} startIcon="user" />,
  // A node goes in the slot made for one.
  <TextInput label="Name" value="" onChange={none} startContent={node} />,
  <Token label="Design" startIcon="tag" />,
];

export const refused = [
  // @ts-expect-error free text, which could be any word
  <Badge label="Paid" startIcon={word} />,
  // @ts-expect-error a node where a name belongs
  <Badge label="Paid" startIcon={node} />,
  // @ts-expect-error
  <DropdownMenuItem label="Delete" icon={node} onClick={none} />,
  // @ts-expect-error
  <DropdownMenuSubmenu label="Move" icon={node}><text>Menu</text></DropdownMenuSubmenu>,
  // @ts-expect-error
  <TextInput label="Name" value="" onChange={none} startIcon={node} />,
  // @ts-expect-error
  <TextInput label="Name" value="" onChange={none} startIcon={word} />,
];
