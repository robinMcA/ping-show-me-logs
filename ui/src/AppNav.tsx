import BluetoothSearching from "@mui/icons-material/BluetoothSearching";
import Search from "@mui/icons-material/Search";
import Box from "@mui/material/Box";
import Button from "@mui/material/Button";
import Divider from "@mui/material/Divider";
import Drawer from "@mui/material/Drawer";
import List from "@mui/material/List";
import ListItem from "@mui/material/ListItem";
import ListItemButton from "@mui/material/ListItemButton";
import ListItemIcon from "@mui/material/ListItemIcon";
import ListItemText from "@mui/material/ListItemText";
import "@xyflow/react/dist/style.css";
import { type ReactNode, useState } from "react";
import { Link } from "react-router";

const DrawerList = ({
  toggleDrawer,
}: {
  toggleDrawer: (input: boolean) => () => void;
}) => (
  <Box sx={{ width: 250 }} role="presentation" onClick={toggleDrawer(false)}>
    <List>
      {[
        ["Search Logs", "search"],
        ["Watch Logs", "watch"],
        ["Flow", "/"],
      ].map(([text, key], index) => (
        <ListItem key={text} disablePadding>
          <Link to={key}>
            <ListItemButton>
              <ListItemIcon>
                {index % 2 === 0 ? <Search /> : <BluetoothSearching />}
              </ListItemIcon>
              <ListItemText primary={text} />
            </ListItemButton>
          </Link>
        </ListItem>
      ))}
    </List>
    <Divider />
  </Box>
);

const AppNav = ({ children }: { children: ReactNode }) => {
  const [open, setOpen] = useState(false);

  const toggleDrawer = (newOpen: boolean) => () => {
    setOpen(newOpen);
  };

  return (
    <div style={{ height: "90vh", width: "97vw" }}>
      <Button onClick={toggleDrawer(true)}>Open Side Menu</Button>
      <Drawer open={open} onClose={toggleDrawer(false)}>
        <DrawerList toggleDrawer={toggleDrawer} />
      </Drawer>
      <>{children}</>
    </div>
  );
};

export default AppNav;
