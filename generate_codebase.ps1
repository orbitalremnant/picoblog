CodeWeaver -h
CodeWeaver -clipboard `
    -include "^src,^templates,^sample_content,Cargo.toml," `
    -output "codebase.md" `
    -excluded-paths-file "codebase_excluded_paths.txt" 
    
    # `
    # -ignore "\.csv,\.pt,\.json,\.ps1,\.txt,\.png,\.html,\.pytest,\.ruff,__pycache__,venv"