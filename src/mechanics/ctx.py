import os

# File extensions to include
valid_extensions = {".rs", ".toml", ".md"}

def read_files_recursively(base_dir="."):
    for root, _, files in os.walk(base_dir):
        for file in files:
            _, ext = os.path.splitext(file)
            if ext.lower() in valid_extensions:
                file_path = os.path.join(root, file)
                try:
                    with open(file_path, "r", encoding="utf-8") as f:
                        content = f.read()
                    print(f"### {file_path}\n")
                    print('```')
                    print(content)
                    print('```\n')
                except Exception as e:
                    print(f"Could not read {file_path}: {e}")

if __name__ == "__main__":
    read_files_recursively(".")
