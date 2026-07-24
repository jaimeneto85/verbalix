import { expect, test } from "@playwright/test";

test("toolbar and note documents are transparent outside their cards", async ({
  page
}) => {
  for (const overlay of ["toolbar", "note"]) {
    await page.goto(`/?overlay=${overlay}&generation=test-generation`);
    const surface = await page.evaluate(() => {
      const root = document.getElementById("root")!;
      return {
        className: document.documentElement.className,
        html: getComputedStyle(document.documentElement).backgroundColor,
        body: getComputedStyle(document.body).backgroundColor,
        root: getComputedStyle(root).backgroundColor,
        minWidth: getComputedStyle(document.body).minWidth,
        overflow: getComputedStyle(root).overflow
      };
    });

    expect(surface).toEqual({
      className: "overlay-surface",
      html: "rgba(0, 0, 0, 0)",
      body: "rgba(0, 0, 0, 0)",
      root: "rgba(0, 0, 0, 0)",
      minWidth: "0px",
      overflow: "hidden"
    });
  }
});

test("an overlay route without a Rust generation fails closed", async ({
  page
}) => {
  await page.goto("/?overlay=toolbar");

  await expect(page.locator("html")).toHaveClass("overlay-surface");
  await expect(page.locator(".toolbar")).toHaveCount(0);
  await expect(page.getByText("Acesso ao Verbalix")).toHaveCount(0);
  await expect(page.locator("#root")).toBeEmpty();
});

test("main and unsupported routes preserve the opaque application surface", async ({
  page
}) => {
  for (const path of ["/", "/?overlay=unsupported"]) {
    await page.goto(path);

    await expect(page.locator("html")).not.toHaveClass("overlay-surface");
    expect(
      await page.locator("html").evaluate((element) => {
        return getComputedStyle(element).backgroundColor;
      })
    ).toBe("rgb(238, 241, 245)");
  }
});
