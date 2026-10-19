---
schema: 1
id: local
transport:
  kind: stdio
  command: example-server
  args: []
  env:
    ACCESS_TOKEN: EXAMPLE_ACCESS_TOKEN
---
A definition fixture; checking it must not start the command.
