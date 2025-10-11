# LingXia CLI (draft)

开发者使用的 LingXia 命令行入口，当前仅在本地迭代，尚未发布到 npm。

## 命令

```bash
lingxia build [--dev|--prod]
```

命令逻辑来自内部的 `lingxia-builder` 包。

## 开发

```bash
npm install --prefix ../lingxia-builder
npm install --prefix .
npm run build
```

构建完成后可直接执行：

```bash
./bin/lingxia.js --help
```

如需修改 CLI 行为，请编辑 `src/index.ts` 并重新运行 `npm run build`。
