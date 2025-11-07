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
import { PingNode } from "./CustomNodes.tsx";
import type { Root } from "./types";
import { ReactFlow, useOnSelectionChange } from "@xyflow/react";
import "@xyflow/react/dist/style.css";

const pages = ["manualLogs", "watchLogs", "flow"] as const;

const simpleJsonFetcher: Fetcher<Root, string> = (url: string) =>
  fetch(url).then((r) => r.json());

const jsonFetcher = (url: string) => fetch(url).then((r) => r.json());

const DrawerList = (
  toggleDrawer: (state: boolean) => () => void,
  togglePage: (pageKey: (typeof pages)[number]) => void,
) => (
  <Box sx={{ width: 250 }} role="presentation" onClick={toggleDrawer(false)}>
    <List>
      {[
        ["Manual Logs", "manualLogs"],
        ["Watch Logs", "watchLogs"],
        ["Flow", "flow"],
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

const nodeTypes = {
  ping: PingNode,
};

const ReactFlowComp = () => {
  const [startsWith, setStartsWith] = useState<string>("");
  const [endsWith, setEndsWith] = useState<string>("");

  const urlSearch = new URLSearchParams({
    starts_with: startsWith,
    ends_with: endsWith,
  });
  const { data: journeyList } = useSWR(
    `${document.URL.includes("5173") ? "http://localhost:8081" : ""}/api/journey?${urlSearch.toString()}`,
    jsonFetcher,
  );
  const [selectedJourney, setReselectedJourney] = useState<string>();

  const [selectedNode, setSelectedNode] = useState<string | undefined>(
    undefined,
  );
  const [transactionId, setTransactionId] = useState<string | undefined>(
    undefined,
  );

  useOnSelectionChange({
    onChange: (data) => {
      const node = data.nodes[0];

      if (!node) {
        return;
      }

      setSelectedNode(node.data.id as string);
    },
  });

  const { data: journeyFlow } = useSWR(
    selectedJourney === undefined
      ? null
      : `${document.URL.includes("5173") ? "http://localhost:8081" : ""}/api/journey/${selectedJourney}/flow${transactionId !== undefined ? `?transaction_id=${transactionId}` : ""}`,
    jsonFetcher,
  );

  const { data: journeyTransactions } = useSWR(
    selectedJourney === undefined
      ? null
      : `${document.URL.includes("5173") ? "http://localhost:8081" : ""}/api/journey/${selectedJourney}/transactions`,
    (url: string) =>
      jsonFetcher(url).then(
        (
          data: {
            transaction_id: string;
            timestamp: string;
          }[],
        ) => [
          ...new Set(
            data
              .sort(({ timestamp: timestampA }, { timestamp: timestampB }) =>
                timestampA > timestampB ? 1 : -1,
              )
              .map(({ transaction_id }) =>
                transaction_id.split("-request")[0].replace(/\/\d/gu, ""),
              ),
          ),
        ],
      ),
  );

  const { data: journeyScripts } = useSWR(
    selectedJourney === undefined
      ? null
      : `${document.URL.includes("5173") ? "http://localhost:8081" : ""}/api/journey/${selectedJourney}/scripts`,
    jsonFetcher,
  );

  console.info({ journeyScripts });

  const nodes =
    journeyScripts &&
    journeyFlow?.nodes.map(
      (node: { id: string; data: { name?: string }; handles: object[] }) => ({
        ...node,
        type: "ping",
        style: {
          height: Math.max(80, node.handles.length * 20 + 20),
        },
        data: {
          handles: node.handles,
          ...node.data,
          scriptContent: journeyScripts[node.id] ?? [{}, { script: "" }],
          name: node.data.name?.startsWith("s")
            ? node.data.name
            : node.data.name === "70e691a5-1e33-4ac3-a356-e7b6d60d92e0"
              ? "Success"
              : node.data.name === "e301438c-0bd0-429c-ab0c-66126501069a"
                ? "Fail"
                : node.data.name,
        },
      }),
    );

  const edges = journeyFlow?.edges.map((e: any) => ({
    ...e,
    markerEnd: "arrow",
    type: "simplebezier",
    animated: e.style.stroke !== "grey",
    style: {
      ...e.style,
      strokeWidth: e.style.stroke !== "grey" ? 10 : undefined,
    },
  }));

  return (
    <div style={{ display: "flex", flexDirection: "row" }}>
      <div style={{ minWidth: "650px" }}>
        <select
          value={transactionId}
          onChange={(event) => setTransactionId(event.target.value)}
        >
          {(journeyTransactions ?? []).map((transactionId, i) => (
            <option key={i} value={transactionId}>
              {transactionId}
            </option>
          ))}
        </select>
        {selectedNode !== undefined ? (
          <p>Select a node to view its logs.</p>
        ) : (
          <p></p>
        )}
      </div>
      <div>
        <select
          value={selectedJourney}
          onChange={(event) => setReselectedJourney(event.target.value)}
        >
          {((journeyList as string[]) ?? []).sort().map((name, i) => (
            <option key={i} value={name}>
              {name}
            </option>
          ))}
        </select>
        <label htmlFor="startsWith">Starts With:</label>
        <input
          type="text"
          id="startsWith"
          name="startsWith"
          value={startsWith}
          onChange={(e) => setStartsWith(e.target.value)}
        />
        <label htmlFor="endsWith">Ends With:</label>
        <input
          type="text"
          id="endsWith"
          name="endsWith"
          value={endsWith}
          onChange={(e) => setEndsWith(e.target.value)}
        />
        <div style={{ height: "90vh", width: "90vw" }}>
          <ReactFlow nodes={nodes} edges={edges} nodeTypes={nodeTypes} />
        </div>
      </div>
    </div>
  );
};

const Page = ({ selectedPage }: { selectedPage?: (typeof pages)[number] }) => {
  switch (selectedPage) {
    case "manualLogs":
      return <ManualLogs />;
    case "watchLogs":
      return <WatchLogs />;
    case "flow":
      return <ReactFlowComp />;
  }
};

function App() {
  const [open, setOpen] = useState(false);

  const [selectedPage, setSelectedPage] =
    useState<(typeof pages)[number]>("flow");

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
