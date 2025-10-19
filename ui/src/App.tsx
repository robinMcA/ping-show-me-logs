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
import { useState } from "react";
import "./App.css";
import useSWR, { type Fetcher } from "swr";
import type { Root } from "./types";

const pages = ["manualLogs", "watchLogs"] as const;

const simpleJsonFetcher: Fetcher<Root, string> = (url: string) =>
  fetch(url).then((r) => r.json());
const DrawerList = (
  toggleDrawer: (state: boolean) => () => void,
  togglePage: (pageKey: (typeof pages)[number]) => void,
) => (
  <Box sx={{ width: 250 }} role="presentation" onClick={toggleDrawer(false)}>
    <List>
      {[
        ["Manual Logs", "manualLogs"],
        ["Watch Logs", "watchLogs"],
      ].map(([text, key], index) => (
        <ListItem key={text} disablePadding>
          {/* eslint-disable-next-line @typescript-eslint/ban-ts-comment */}
          {/*// @ts-expect-error*/}
          <ListItemButton onClick={() => togglePage(key)}>
            <ListItemIcon>
              {index % 2 === 0 ? <Search /> : <BluetoothSearching />}
            </ListItemIcon>
            <ListItemText primary={text} />
          </ListItemButton>
        </ListItem>
      ))}
    </List>
    <Divider />
  </Box>
);

const ManualLogs = () => {
  const [frRequestId, setFrRequestId] = useState<string>();
  const { data } = useSWR(
    frRequestId === undefined
      ? null
      : `${document.URL.includes("5173") ? "http://localhost:8081" : ""}/api/logs/${frRequestId}?filters=Error`,
    simpleJsonFetcher,
  );
  return (
    <>
      <h3>From Form</h3>
      <form className="card">
        <label htmlFor={"frId"}>Fr request id </label>
        <input
          id={"frId"}
          onChange={(event) => setFrRequestId(event.target.value)}
        />
      </form>
      <ol>
        {data?.result.map((res) => (
          <li>{res.payload.message}</li>
        ))}
      </ol>
    </>
  );
};

const WatchLogs = () => {
  const [watching, setWatching] = useState<string>("Error");
  const { data: watchData } = useSWR(
    `${document.URL.includes("5173") ? "http://localhost:8081" : ""}/api/logs/watch?filters=${watching ?? "All"}`,
    simpleJsonFetcher,
  );
  return (
    <>
      <h3>Watching</h3>
      <label>logs types to keep</label>
      <select
        value={watching}
        onChange={(event) => setWatching(event.target.value)}
      >
        <option value={"Warn"}>Warn</option>
        <option value={"All"}>All</option>
        <option value={"Error"}>Error</option>
        <option value={"Debug"}>Debug</option>
      </select>
      <ol>
        {watchData?.result.map((res) => (
          <li>{res.payload.message}</li>
        ))}
      </ol>
    </>
  );
};

const Page = ({ selectedPage }: { selectedPage?: (typeof pages)[number] }) => {
  switch (selectedPage) {
    case "manualLogs":
      return <ManualLogs />;
    case "watchLogs":
      return <WatchLogs />;
  }
};

function App() {
  const [open, setOpen] = useState(false);

  const [selectedPage, setSelectedPage] =
    useState<(typeof pages)[number]>("watchLogs");

  const toggleDrawer = (newOpen: boolean) => () => {
    setOpen(newOpen);
  };

  return (
    <>
      <Button onClick={toggleDrawer(true)}>Open Side Menu</Button>
      <Drawer open={open} onClose={toggleDrawer(false)}>
        {DrawerList(toggleDrawer, setSelectedPage)}
      </Drawer>
      <Box>
        <Page selectedPage={selectedPage} />
      </Box>
    </>
  );
}

export default App;
